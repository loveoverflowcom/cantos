//! Isolated Theatre mock UI. These commands exercise presentation fixtures, never a listener
//! API, native audio engine, download, account or production mobile application.

use std::{
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use anyhow::{bail, Result};
use clap::Subcommand;

use crate::{
    command::{CommandSpec, Runner},
    config::THEATRE_MOBILE_DIR,
    gate::Gate,
    pipeline::{self, Step},
    process::{Supervisor, Wait},
    report::{Report, Status},
    tools::{cmp_gradle_project_exists, Tool, ToolProbe},
    workspace::Workspace,
};

const RUN_JVM_ARGS: &str = "-Xms64m -Xmx2g -Dfile.encoding=UTF-8";
const KOTLIN_NATIVE_VERSION: &str = "2.2.20";
const ANDROID_PLATFORM: &str = "platforms/android-35/android.jar";
pub const ANDROID_BUILD_TOOLS: &str = "build-tools/35.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum DemoAction {
    /// Compile the desktop host for the shared mock screens.
    Build,
    /// Run deterministic state and rendered desktop UI tests.
    Verify,
    /// Supervise the mock desktop window; no audio or native device is started.
    Run,
    /// Compile an Android debug mock app using an already installed Android SDK.
    AndroidBuild,
    /// Link the mock iOS simulator framework with an installed pinned native compiler and Xcode.
    IosBuild {
        /// Absolute path to an already installed Kotlin/Native 2.2.20 distribution.
        #[arg(long, value_name = "PATH")]
        kotlin_native_home: PathBuf,
    },
}

/// Both KMP modules must exist: production-safe shared code and the explicit demo sandbox.
pub fn project_exists(dir: &Path) -> bool {
    cmp_gradle_project_exists(dir)
        && dir.join("shared/build.gradle.kts").is_file()
        && dir.join("shared/src/commonMain/kotlin").is_dir()
        && dir.join("preview-sandbox/build.gradle.kts").is_file()
        && dir.join("preview-sandbox/src/commonMain/kotlin").is_dir()
        && dir.join("preview-sandbox/src/desktopMain/kotlin").is_dir()
}

pub fn tools() -> Vec<Tool> {
    vec![Tool::Gradle, Tool::Java, Tool::TheatreDemoGradleProject]
}

fn gradle_command(workspace: &Workspace) -> CommandSpec {
    CommandSpec::new("gradle", workspace.root()).args([
        "--project-dir",
        THEATRE_MOBILE_DIR,
        "--no-daemon",
        "-Dorg.gradle.java.installations.auto-download=false",
        "--max-workers",
        "2",
        "-Pkotlin.compiler.execution.strategy=in-process",
        "--no-configuration-cache",
    ])
}

/// Match the launch JVM to Gradle's requested JVM so run cannot detach from its owned group.
/// The sandbox Gradle module checks this launcher PID before executing the desktop task.
pub fn desktop_command(workspace: &Workspace, task: &str) -> CommandSpec {
    let mut command = gradle_command(workspace);
    if task == "desktopTest" {
        // Moving mock bindings must never drop the independently runnable shared-core tests.
        command = command.args([":shared:desktopTest"]);
    }
    let command = command.args([format!(":preview-sandbox:{task}")]);
    if task != "run" {
        return command;
    }
    let script = format!(
        "export JAVA_OPTS=\"{RUN_JVM_ARGS} -Dcantos.theatre.demo.launcherPid=$$\"; exec gradle \"$@\""
    );
    CommandSpec::new("/bin/sh", workspace.root())
        .args(["-c", &script, "cantos-theatre-demo"])
        .args(command.args)
        .args([format!("-Dorg.gradle.jvmargs={RUN_JVM_ARGS}")])
        .env(
            "GRADLE_OPTS",
            "-Dorg.gradle.internal.instrumentation.agent=true",
        )
}

/// The one Gradle invocation that builds the Android debug mock; distribution reuses it as is.
pub fn android_command(workspace: &Workspace) -> CommandSpec {
    gradle_command(workspace).args([
        "-Pcantos.android=true",
        "-Pandroid.builder.sdkDownload=false",
        ":androidApp:assembleDebug",
    ])
}

/// The already installed Android SDK named by the environment, if any; never downloaded.
pub fn android_sdk_from_environment() -> Option<PathBuf> {
    std::env::var_os("ANDROID_HOME")
        .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
        .map(PathBuf::from)
}

fn ios_command(workspace: &Workspace, compiler_home: &str) -> CommandSpec {
    gradle_command(workspace).args([
        "-Pcantos.ios=true".to_owned(),
        format!("-Pkotlin.native.home={compiler_home}"),
        ":preview-sandbox:linkDebugFrameworkIosSimulatorArm64".to_owned(),
    ])
}

fn unsupported(subject: &str, reason: impl Into<String>) -> ExitCode {
    let mut report = Report::new("theatre-demo");
    report.record(subject, Status::Unsupported, reason);
    report.print();
    report.exit_code()
}

fn native_compiler_prerequisite(compiler_home: &Path) -> Result<(), &'static str> {
    if !compiler_home.is_absolute() || compiler_home.to_str().is_none() {
        return Err("--kotlin-native-home must be an absolute UTF-8 path to an installed compiler");
    }
    if !local_executable(&compiler_home.join("bin/konanc")) {
        return Err("installed Kotlin/Native compiler is missing executable bin/konanc; xtask never downloads it");
    }
    if !compiler_home
        .join("klib/common/stdlib/default/manifest")
        .is_file()
    {
        return Err("installed Kotlin/Native compiler has no stdlib manifest");
    }
    // The stdlib manifest records its build compiler, which can be an RC even in a stable
    // distribution. Ask the installed compiler for its distribution version without a target.
    let output = Command::new(compiler_home.join("bin/konanc"))
        .arg("-version")
        .output()
        .map_err(|_| "installed Kotlin/Native compiler could not report its version")?;
    if !output.status.success() || !pinned_native_version(&output.stdout, &output.stderr) {
        return Err("ios-build requires an already installed Kotlin/Native 2.2.20 compiler");
    }
    Ok(())
}

fn pinned_native_version(stdout: &[u8], stderr: &[u8]) -> bool {
    [stdout, stderr].into_iter().find_map(|bytes| {
        String::from_utf8_lossy(bytes).lines().find_map(|line| {
            line.trim()
                .strip_prefix("Kotlin/Native:")
                .map(|version| version.trim() == KOTLIN_NATIVE_VERSION)
        })
    }) == Some(true)
}

pub fn local_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

pub fn android_prerequisite(project: &Path, sdk: Option<&Path>) -> Result<(), &'static str> {
    if !project.join("androidApp/build.gradle.kts").is_file()
        || !project
            .join("androidApp/src/main/AndroidManifest.xml")
            .is_file()
        || !project.join("shared/src/androidMain/kotlin").is_dir()
        || !project
            .join("preview-sandbox/src/androidMain/kotlin")
            .is_dir()
    {
        return Err("Android mock Gradle module, manifest or shared Android sources are missing");
    }
    let sdk =
        sdk.ok_or("set ANDROID_HOME or ANDROID_SDK_ROOT to an already installed Android SDK")?;
    if !sdk.is_absolute() {
        return Err("configured Android SDK must be an absolute path, independent of the caller's directory");
    }
    if !sdk.is_dir() {
        return Err("configured Android SDK directory does not exist; xtask never installs an SDK");
    }
    if !sdk.join(ANDROID_PLATFORM).is_file()
        || !local_executable(&sdk.join(ANDROID_BUILD_TOOLS).join("aapt2"))
        || !local_executable(&sdk.join(ANDROID_BUILD_TOOLS).join("d8"))
    {
        return Err("requires installed Android SDK platform 35 and build tools 35.0.0; SDK downloading is disabled");
    }
    Ok(())
}

/// A valid simulator SDK distinguishes full Xcode from command-line tools alone. This probe
/// reads installed tool output only and cannot install or configure Xcode.
fn ios_sdk_available() -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    Command::new("xcrun")
        .args(["--sdk", "iphonesimulator", "--show-sdk-path"])
        .output()
        .ok()
        .is_some_and(|output| {
            output.status.success()
                && Path::new(String::from_utf8_lossy(&output.stdout).trim()).is_dir()
        })
}

fn launch(workspace: &Workspace) -> Result<ExitCode> {
    let command = desktop_command(workspace, "run");
    println!(
        "\n==> start Theatre mock UI: {}",
        command.display(workspace.root())
    );
    let mut supervisor = Supervisor::new()?;
    supervisor.spawn("Theatre mock desktop UI", &command)?;
    println!(
        "Mock Gradle desktop task started. Window readiness is not asserted. Press Ctrl-C to stop."
    );
    let outcome = supervisor.wait()?;
    supervisor.shutdown();
    match outcome {
        Wait::Interrupted => {
            println!("Stopped the owned mock process group; complete shutdown requires runtime inspection.");
            Ok(ExitCode::SUCCESS)
        }
        Wait::Exited { status, .. } if status.success() => Ok(ExitCode::SUCCESS),
        Wait::Exited { name, status } => bail!("{name} exited with {status}"),
    }
}

pub fn execute(
    workspace: &Workspace,
    action: DemoAction,
    tool_probe: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let steps = match action {
        DemoAction::Build => vec![Step::new(
            "Theatre mock desktop build",
            &tools(),
            vec![desktop_command(workspace, "compileKotlinDesktop")],
        )],
        DemoAction::Verify => vec![Gate::TheatreCmpDemo.step(workspace)],
        DemoAction::AndroidBuild => {
            let sdk = android_sdk_from_environment();
            if let Err(reason) =
                android_prerequisite(&workspace.path(THEATRE_MOBILE_DIR), sdk.as_deref())
            {
                return Ok(unsupported("Theatre mock Android build", reason));
            }
            vec![Step::new(
                "Theatre mock Android build",
                &tools(),
                vec![android_command(workspace)],
            )]
        }
        DemoAction::IosBuild { kotlin_native_home } => {
            if let Err(reason) = native_compiler_prerequisite(&kotlin_native_home) {
                return Ok(unsupported("Theatre mock iOS simulator framework", reason));
            }
            if !workspace
                .path(THEATRE_MOBILE_DIR)
                .join("preview-sandbox/src/iosMain/kotlin")
                .is_dir()
            {
                return Ok(unsupported(
                    "Theatre mock iOS simulator framework",
                    "sandbox iOS mock sources are missing",
                ));
            }
            if !ios_sdk_available() {
                return Ok(unsupported("Theatre mock iOS simulator framework", "requires full installed Xcode with a usable iOS simulator SDK; xtask never installs or selects Xcode"));
            }
            // The prerequisite already rejected paths which cannot become an exact argv value.
            let compiler_home = kotlin_native_home.to_string_lossy();
            vec![Step::new(
                "Theatre mock iOS simulator framework",
                &tools(),
                vec![ios_command(workspace, &compiler_home)],
            )]
        }
        DemoAction::Run => {
            if !cfg!(unix) {
                return Ok(unsupported(
                    "Theatre mock desktop run",
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
                    "Theatre mock desktop run",
                    format!("missing {}", details.join(", ")),
                ));
            }
            return launch(workspace);
        }
    };
    let report = pipeline::execute("theatre-demo", &steps, tool_probe, runner);
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

    fn executable_fixture(path: &Path) {
        std::fs::write(path, "fixture executable; never executed").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    #[test]
    fn mock_commands_name_exact_tasks_and_disable_toolchain_provisioning() {
        let workspace = workspace();
        assert_eq!(
            desktop_command(&workspace, "compileKotlinDesktop").display(workspace.root()),
            "gradle --project-dir apps/cantos-theatre/mobile --no-daemon -Dorg.gradle.java.installations.auto-download=false --max-workers 2 -Pkotlin.compiler.execution.strategy=in-process --no-configuration-cache :preview-sandbox:compileKotlinDesktop"
        );
        assert_eq!(
            Gate::TheatreCmpDemo.commands(&workspace),
            [desktop_command(&workspace, "desktopTest")]
        );
        assert!(desktop_command(&workspace, "desktopTest").args.ends_with(&[
            ":shared:desktopTest".to_owned(),
            ":preview-sandbox:desktopTest".to_owned(),
        ]));
        let android = android_command(&workspace);
        assert!(android.args.ends_with(&[
            "-Pcantos.android=true".to_owned(),
            "-Pandroid.builder.sdkDownload=false".to_owned(),
            ":androidApp:assembleDebug".to_owned()
        ]));
        let ios = ios_command(&workspace, "/tmp/compiler home");
        assert!(ios.args.ends_with(&[
            "-Pcantos.ios=true".to_owned(),
            "-Pkotlin.native.home=/tmp/compiler home".to_owned(),
            ":preview-sandbox:linkDebugFrameworkIosSimulatorArm64".to_owned()
        ]));
        for command in [android, ios, desktop_command(&workspace, "desktopTest")] {
            assert_eq!(command.program, "gradle");
            assert!(command.env.is_empty());
            assert!(!command.args.iter().any(|arg| arg.contains("cantos-api")
                || arg.contains("cantos-studio")
                || arg.contains("wrapper")));
        }
    }

    #[test]
    fn run_uses_only_owned_launcher_with_exact_jvm_match_and_fresh_configuration() {
        let command = desktop_command(&workspace(), "run");
        assert_eq!(command.program, "/bin/sh");
        assert_eq!(command.args[..3], ["-c", "export JAVA_OPTS=\"-Xms64m -Xmx2g -Dfile.encoding=UTF-8 -Dcantos.theatre.demo.launcherPid=$$\"; exec gradle \"$@\"", "cantos-theatre-demo"]);
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
    fn every_missing_mock_prerequisite_prevents_build_test_and_launch() {
        for tool in tools() {
            for action in [DemoAction::Build, DemoAction::Verify, DemoAction::Run] {
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
    fn missing_native_compiler_is_unsupported_without_configuring_gradle() {
        let mut runner = RecordingRunner::default();
        assert_eq!(
            execute(
                &workspace(),
                DemoAction::IosBuild {
                    kotlin_native_home: PathBuf::from("missing-relative-compiler")
                },
                &FakeTools::all(),
                &mut runner
            )
            .unwrap(),
            ExitCode::FAILURE
        );
        assert!(runner.commands.is_empty());
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
    #[cfg(unix)]
    fn native_probe_uses_distribution_version_instead_of_stdlib_build_version() {
        let dir = std::env::temp_dir().join(format!("xtask-theatre-native-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        assert!(native_compiler_prerequisite(&dir)
            .unwrap_err()
            .contains("bin/konanc"));
        let compiler = dir.join("bin/konanc");
        executable_fixture(&compiler);
        let manifest_dir = dir.join("klib/common/stdlib/default");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(
            manifest_dir.join("manifest"),
            "compiler_version=2.2.20-RC3-275\n",
        )
        .unwrap();
        std::fs::write(
            &compiler,
            "#!/bin/sh\nprintf '%s\\n' 'Kotlin/Native: 2.4.20'\n",
        )
        .unwrap();
        assert!(native_compiler_prerequisite(&dir)
            .unwrap_err()
            .contains("2.2.20"));
        std::fs::write(
            &compiler,
            "#!/bin/sh\nprintf '%s\\n' 'Kotlin/Native: 2.2.20'\n",
        )
        .unwrap();
        assert_eq!(native_compiler_prerequisite(&dir), Ok(()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_distribution_version_parser_checks_both_streams_and_exact_pin() {
        for (stdout, stderr) in [
            (b"Kotlin/Native: 2.2.20\n".as_slice(), b"".as_slice()),
            (
                b"".as_slice(),
                b"info: kotlinc-native 2.2.20 (JRE 21)\nKotlin/Native: 2.2.20\n".as_slice(),
            ),
        ] {
            assert!(pinned_native_version(stdout, stderr));
        }
        for line in [
            "Kotlin/Native: 2.4.20",
            "Kotlin/Native: 2.2.20-dev-1",
            "compiler_version=2.2.20",
            "",
        ] {
            assert!(!pinned_native_version(line.as_bytes(), b""));
        }
    }

    #[test]
    fn android_missing_project_or_sdk_never_passes_prerequisites() {
        let dir =
            std::env::temp_dir().join(format!("xtask-theatre-android-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(android_prerequisite(&dir, None)
            .unwrap_err()
            .contains("module"));
        std::fs::create_dir_all(dir.join("androidApp/src/main")).unwrap();
        std::fs::create_dir_all(dir.join("shared/src/androidMain/kotlin")).unwrap();
        std::fs::create_dir_all(dir.join("preview-sandbox/src/androidMain/kotlin")).unwrap();
        std::fs::write(dir.join("androidApp/build.gradle.kts"), "").unwrap();
        std::fs::write(dir.join("androidApp/src/main/AndroidManifest.xml"), "").unwrap();
        assert!(android_prerequisite(&dir, None)
            .unwrap_err()
            .contains("ANDROID_HOME"));
        assert!(android_prerequisite(&dir, Some(&dir.join("absent-sdk")))
            .unwrap_err()
            .contains("does not exist"));
        assert!(android_prerequisite(&dir, Some(&dir))
            .unwrap_err()
            .contains("platform 35"));
        std::fs::create_dir_all(dir.join("platforms/android-35")).unwrap();
        std::fs::create_dir_all(dir.join(ANDROID_BUILD_TOOLS)).unwrap();
        std::fs::write(dir.join(ANDROID_PLATFORM), "").unwrap();
        executable_fixture(&dir.join(ANDROID_BUILD_TOOLS).join("aapt2"));
        executable_fixture(&dir.join(ANDROID_BUILD_TOOLS).join("d8"));
        assert_eq!(android_prerequisite(&dir, Some(&dir)), Ok(()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
