//! Theatre Desktop Preview. It mounts the one `CantosTheatreApp` over mock external ports (public
//! catalog, playback access, decoder stand-in, identity provider and remote Library) on the JVM
//! desktop. These commands never exercise a listener API, a native audio engine, a download, a
//! real account or a production mobile application, and there is no Android or iOS preview host.

use std::{path::Path, process::ExitCode};

use anyhow::{bail, Result};
use clap::Subcommand;

use crate::{
    command::{CommandSpec, Runner},
    gate::Gate,
    mobile_toolchain,
    pipeline::{self, Step},
    process::{Supervisor, Wait},
    report::{Report, Status},
    tools::{cmp_gradle_project_exists, Tool, ToolProbe},
    workspace::Workspace,
};

const RUN_JVM_ARGS: &str = "-Xms64m -Xmx2g -Dfile.encoding=UTF-8";

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum TheatrePreviewAction {
    /// Compile the Desktop Preview host.
    Build,
    /// Run deterministic state and rendered desktop UI tests (the shared core and the sandbox).
    Verify,
    /// Supervise the Desktop Preview window; no audio, network or native device is started.
    Run,
    /// Retired (#286): the Android mock host was merged into the one real-only `:androidApp`.
    AndroidBuild,
    /// Retired (#287): the iOS mock host and its `CantosTheatreDemo` framework no longer exist.
    IosBuild,
}

/// The shared core and the JVM-only sandbox must both exist as KMP modules with desktop sources.
pub fn project_exists(dir: &Path) -> bool {
    cmp_gradle_project_exists(dir)
        && dir.join("shared/build.gradle.kts").is_file()
        && dir.join("shared/src/commonMain/kotlin").is_dir()
        && dir.join("preview-sandbox/build.gradle.kts").is_file()
        && dir.join("preview-sandbox/src/commonMain/kotlin").is_dir()
        && dir.join("preview-sandbox/src/desktopMain/kotlin").is_dir()
}

pub fn tools() -> Vec<Tool> {
    vec![Tool::Gradle, Tool::Java, Tool::TheatrePreviewGradleProject]
}

/// Match the launch JVM to Gradle's requested JVM so run cannot detach from its owned group.
/// The sandbox Gradle module checks this launcher PID before executing the desktop task.
pub fn desktop_command(workspace: &Workspace, task: &str) -> CommandSpec {
    let mut command = mobile_toolchain::gradle_command(workspace);
    if task == "desktopTest" {
        // Moving mock bindings must never drop the independently runnable shared-core tests.
        command = command.args([":shared:desktopTest"]);
    }
    let command = command.args([format!(":preview-sandbox:{task}")]);
    if task != "run" {
        return command;
    }
    let script = format!(
        "export JAVA_OPTS=\"{RUN_JVM_ARGS} -Dcantos.theatre.preview.launcherPid=$$\"; exec gradle \"$@\""
    );
    CommandSpec::new("/bin/sh", workspace.root())
        .args(["-c", &script, "cantos-theatre-preview"])
        .args(command.args)
        .args([format!("-Dorg.gradle.jvmargs={RUN_JVM_ARGS}")])
        .env(
            "GRADLE_OPTS",
            "-Dorg.gradle.internal.instrumentation.agent=true",
        )
}

/// Why `theatre-preview android-build` builds nothing. Refused before any tool probe.
pub const RETIRED_ANDROID_BUILD: &str = "retired by #286: the Android mock host no longer exists; \
:androidApp is the one real-only application. Use `cargo xtask theatre-host android-debug \
--api-origin ORIGIN` for the debug build (com.cantos.theatre.dev); the only preview is the JVM Desktop Preview";

/// Why `theatre-preview ios-build` builds nothing. Refused before any tool probe.
pub const RETIRED_IOS_BUILD: &str = "retired by #287: the iOS mock host and its CantosTheatreDemo \
framework no longer exist; the sandbox is JVM desktop only. Use `cargo xtask theatre-host ios-build \
--kotlin-native-home PATH` for the production framework; the only preview is the JVM Desktop Preview";

fn unsupported(subject: &str, reason: impl Into<String>) -> ExitCode {
    let mut report = Report::new("theatre-preview");
    report.record(subject, Status::Unsupported, reason);
    report.print();
    report.exit_code()
}

fn launch(workspace: &Workspace) -> Result<ExitCode> {
    let command = desktop_command(workspace, "run");
    println!(
        "\n==> start Theatre Desktop Preview: {}",
        command.display(workspace.root())
    );
    let mut supervisor = Supervisor::new()?;
    supervisor.spawn("Theatre Desktop Preview", &command)?;
    println!(
        "Gradle desktop task started. Window readiness is not asserted. Press Ctrl-C to stop."
    );
    let outcome = supervisor.wait()?;
    supervisor.shutdown();
    match outcome {
        Wait::Interrupted => {
            println!("Stopped the owned preview process group; complete shutdown requires runtime inspection.");
            Ok(ExitCode::SUCCESS)
        }
        Wait::Exited { status, .. } if status.success() => Ok(ExitCode::SUCCESS),
        Wait::Exited { name, status } => bail!("{name} exited with {status}"),
    }
}

pub fn execute(
    workspace: &Workspace,
    action: TheatrePreviewAction,
    tool_probe: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let steps = match action {
        TheatrePreviewAction::Build => vec![Step::new(
            "Theatre Desktop Preview build",
            &tools(),
            vec![desktop_command(workspace, "compileKotlinDesktop")],
        )],
        TheatrePreviewAction::Verify => vec![Gate::TheatreCmpPreview.step(workspace)],
        TheatrePreviewAction::AndroidBuild => {
            return Ok(unsupported(
                "Theatre preview Android build",
                RETIRED_ANDROID_BUILD,
            ))
        }
        TheatrePreviewAction::IosBuild => {
            return Ok(unsupported("Theatre preview iOS build", RETIRED_IOS_BUILD))
        }
        TheatrePreviewAction::Run => {
            if !cfg!(unix) {
                return Ok(unsupported(
                    "Theatre Desktop Preview run",
                    "requires a POSIX exec chain and Unix process-group supervision",
                ));
            }
            let missing = tool_probe.missing(&tools());
            if !missing.is_empty() {
                let details: Vec<_> = missing
                    .iter()
                    .map(|tool| format!("{} ({})", tool.label(), tool.hint()))
                    .collect();
                return Ok(unsupported(
                    "Theatre Desktop Preview run",
                    format!("missing {}", details.join(", ")),
                ));
            }
            return launch(workspace);
        }
    };
    let report = pipeline::execute("theatre-preview", &steps, tool_probe, runner);
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
    fn preview_commands_name_exact_tasks_and_disable_toolchain_provisioning() {
        let workspace = workspace();
        assert_eq!(
            desktop_command(&workspace, "compileKotlinDesktop").display(workspace.root()),
            "gradle --project-dir apps/cantos-theatre/mobile --no-daemon -Dorg.gradle.java.installations.auto-download=false --max-workers 2 -Pkotlin.compiler.execution.strategy=in-process --no-configuration-cache :preview-sandbox:compileKotlinDesktop"
        );
        assert_eq!(
            Gate::TheatreCmpPreview.commands(&workspace),
            [desktop_command(&workspace, "desktopTest")]
        );
        let test = desktop_command(&workspace, "desktopTest");
        assert!(test.args.ends_with(&[
            ":shared:desktopTest".to_owned(),
            ":preview-sandbox:desktopTest".to_owned(),
        ]));
        // Desktop-only: neither an Android SDK nor Kotlin/Native opt-in may reach a preview command.
        for command in [desktop_command(&workspace, "compileKotlinDesktop"), test] {
            assert_eq!(command.program, "gradle");
            assert!(command.env.is_empty());
            assert!(
                !command.args.iter().any(|arg| arg.contains("cantos.android")
                    || arg.contains("cantos.ios")
                    || arg.contains("cantos.native")
                    || arg.contains("kotlin.native.home")
                    || arg.contains("cantos-api")
                    || arg.contains("cantos-studio")
                    || arg.contains("wrapper"))
            );
        }
    }

    #[test]
    fn run_uses_only_owned_launcher_with_exact_jvm_match_and_fresh_configuration() {
        let command = desktop_command(&workspace(), "run");
        assert_eq!(command.program, "/bin/sh");
        assert_eq!(command.args[..3], ["-c", "export JAVA_OPTS=\"-Xms64m -Xmx2g -Dfile.encoding=UTF-8 -Dcantos.theatre.preview.launcherPid=$$\"; exec gradle \"$@\"", "cantos-theatre-preview"]);
        assert!(command.args.contains(&":preview-sandbox:run".to_owned()));
        assert!(!command.args.contains(&":shared:run".to_owned()));
        assert!(command
            .args
            .contains(&format!("-Dorg.gradle.jvmargs={RUN_JVM_ARGS}")));
        assert!(command
            .args
            .contains(&"--no-configuration-cache".to_owned()));
        assert_eq!(
            command.env,
            [(
                "GRADLE_OPTS".to_owned(),
                "-Dorg.gradle.internal.instrumentation.agent=true".to_owned()
            )]
        );
    }

    #[test]
    fn every_missing_preview_prerequisite_prevents_build_test_and_launch() {
        for tool in tools() {
            for action in [
                TheatrePreviewAction::Build,
                TheatrePreviewAction::Verify,
                TheatrePreviewAction::Run,
            ] {
                let mut runner = RecordingRunner::default();
                assert_eq!(
                    execute(
                        &workspace(),
                        action,
                        &FakeTools::without(&[tool]),
                        &mut runner
                    )
                    .unwrap(),
                    ExitCode::FAILURE
                );
                assert!(runner.commands.is_empty());
            }
        }
    }

    #[test]
    fn project_probe_requires_real_module_and_source_sets() {
        let dir =
            std::env::temp_dir().join(format!("xtask-theatre-project-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!project_exists(&dir));
        std::fs::write(dir.join("settings.gradle.kts"), "").unwrap();
        std::fs::write(dir.join("build.gradle.kts"), "").unwrap();
        assert!(!project_exists(&dir));
        std::fs::create_dir_all(dir.join("shared/src/commonMain/kotlin")).unwrap();
        std::fs::write(dir.join("shared/build.gradle.kts"), "").unwrap();
        assert!(!project_exists(&dir));
        std::fs::create_dir_all(dir.join("preview-sandbox/src/commonMain/kotlin")).unwrap();
        std::fs::create_dir_all(dir.join("preview-sandbox/src/desktopMain/kotlin")).unwrap();
        assert!(!project_exists(&dir));
        std::fs::write(dir.join("preview-sandbox/build.gradle.kts"), "").unwrap();
        assert!(project_exists(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn retired_native_preview_lanes_are_refused_before_any_tool_or_command() {
        let workspace = workspace();
        for (action, reason, owner) in [
            (
                TheatrePreviewAction::AndroidBuild,
                RETIRED_ANDROID_BUILD,
                "#286",
            ),
            (TheatrePreviewAction::IosBuild, RETIRED_IOS_BUILD, "#287"),
        ] {
            let mut runner = RecordingRunner::default();
            let code = execute(&workspace, action, &FakeTools::all(), &mut runner).unwrap();
            assert_ne!(code, ExitCode::SUCCESS);
            assert!(runner.commands.is_empty());
            assert!(reason.contains(owner) && reason.contains("theatre-host"));
        }
    }

    #[test]
    fn the_preview_never_resolves_a_native_toolchain() {
        // The sandbox is JVM desktop only: no preview command builds, links or signs anything native.
        let source = include_str!("theatre_preview.rs");
        let code = source.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "konanc",
            "xcrun",
            "ANDROID_HOME",
            "assembleDebug",
            "linkDebugFramework",
        ] {
            assert!(!code.contains(forbidden), "{forbidden}");
        }
    }
}
