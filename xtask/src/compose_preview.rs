//! Optional Android Studio host rendering. The Python primitive owns capture and validation;
//! this module owns the command surface and maps its exit status into xtask's vocabulary.

use std::{
    path::PathBuf,
    process::{ExitCode, ExitStatus},
};

use anyhow::{bail, Result};
use clap::Args;

use crate::{
    command::CommandSpec,
    config::App,
    process::{Supervisor, Wait},
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
        match execute(&spec) {
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

fn execute(spec: &CommandSpec) -> Result<ExitStatus> {
    let mut supervisor = Supervisor::new()?;
    supervisor.spawn("compose-preview capture", spec)?;
    let outcome = supervisor.wait()?;
    supervisor.shutdown();
    match outcome {
        Wait::Exited { status, .. } => Ok(status),
        Wait::Interrupted => bail!("capture interrupted"),
    }
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

    #[cfg(unix)]
    mod lifecycle {
        use super::*;
        use std::{
            fs,
            os::unix::{fs::DirBuilderExt, process::CommandExt},
            path::Path,
            process::{Child, Command, Stdio},
            thread,
            time::{Duration, Instant, SystemTime, UNIX_EPOCH},
        };

        const DRIVER_DIRECTORY: &str = "CANTOS_COMPOSE_CAPTURE_TEST_DIRECTORY";
        const DRIVER_NAME: &str = "compose_preview::tests::lifecycle::capture_driver";
        const WAIT: Duration = Duration::from_secs(5);
        const CAPTURE: &str = r#"
import os, pathlib, signal, subprocess, sys, time
root = pathlib.Path(sys.argv[1])
cli = subprocess.Popen([sys.executable, '-c', '''
import pathlib, signal, sys, time
root = pathlib.Path(sys.argv[1])
def stop(signum, frame):
    (root / 'cli-stopped').write_text(str(signum))
    raise SystemExit(0)
for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
    signal.signal(sig, stop)
(root / 'cli-ready').write_text('ready')
time.sleep(60)
''', str(root)], start_new_session=True)
def stop(signum, frame):
    (root / 'capture-stopped').write_text(str(signum))
    raise SystemExit(1)
for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
    signal.signal(sig, stop)
(root / 'ready').write_text(f'{os.getpid()} {cli.pid}')
try:
    time.sleep(60)
finally:
    os.killpg(cli.pid, signal.SIGTERM)
    cli.wait(timeout=3)
"#;

        struct CaptureTree {
            directory: PathBuf,
            driver: Child,
        }

        impl CaptureTree {
            fn start() -> Self {
                Self::with_script(CAPTURE)
            }

            fn with_script(script: &str) -> Self {
                let nonce = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let directory = std::env::temp_dir().join(format!(
                    "cantos-compose-capture-{}-{nonce}",
                    std::process::id()
                ));
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&directory)
                    .unwrap();
                fs::write(directory.join("capture.py"), script).unwrap();
                let driver = Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", DRIVER_NAME, "--nocapture"])
                    .env(DRIVER_DIRECTORY, &directory)
                    .process_group(0)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap();
                Self { directory, driver }
            }

            fn wait_for(&self, name: &str) {
                let deadline = Instant::now() + WAIT;
                while !self.directory.join(name).is_file() {
                    assert!(Instant::now() < deadline, "capture did not write {name}");
                    thread::sleep(Duration::from_millis(20));
                }
            }

            fn wait_for_driver(&mut self) -> ExitStatus {
                let deadline = Instant::now() + WAIT;
                loop {
                    if let Some(status) = self.driver.try_wait().unwrap() {
                        return status;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "Rust driver survived the stop signal"
                    );
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }

        impl Drop for CaptureTree {
            fn drop(&mut self) {
                // Only PIDs written by this private fixture and the test child we started.
                // Cleanup also runs when the unsupervised baseline fails the assertions.
                // A written outcome follows the synchronous capture wait, including CLI reaping.
                // Avoid signalling already-reaped fixture PIDs on the successful path.
                if let Some(text) = fs::read_to_string(self.directory.join("ready"))
                    .ok()
                    .filter(|_| !self.directory.join("outcome").is_file())
                {
                    let pids: Vec<i32> = text
                        .split_whitespace()
                        .filter_map(|p| p.parse().ok())
                        .collect();
                    for pid in pids {
                        unsafe {
                            libc::kill(-pid, libc::SIGKILL);
                            libc::kill(pid, libc::SIGKILL);
                        }
                    }
                }
                if !matches!(self.driver.try_wait(), Ok(Some(_))) {
                    if let Ok(pid) = i32::try_from(self.driver.id()) {
                        unsafe {
                            libc::kill(-pid, libc::SIGKILL);
                        }
                    }
                }
                let _ = self.driver.kill();
                let _ = self.driver.wait();
                let _ = fs::remove_dir_all(&self.directory);
            }
        }

        fn assert_signal_cleanup(signal: i32) {
            let mut capture = CaptureTree::start();
            capture.wait_for("ready");
            capture.wait_for("cli-ready");
            let pid = i32::try_from(capture.driver.id()).unwrap();
            assert_eq!(unsafe { libc::kill(pid, signal) }, 0);
            let status = capture.wait_for_driver();
            assert!(
                capture.directory.join("capture-stopped").is_file(),
                "Rust PID signal {signal} did not stop its Python capture; driver exited {status}"
            );
            assert!(
                capture.directory.join("cli-stopped").is_file(),
                "Rust PID signal {signal} left the capture's owned CLI alive"
            );
            assert_eq!(
                fs::read_to_string(capture.directory.join("outcome")).unwrap(),
                "error capture interrupted"
            );
        }

        #[test]
        fn capture_driver() {
            let Some(directory) = std::env::var_os(DRIVER_DIRECTORY) else {
                return;
            };
            let directory = Path::new(&directory);
            let spec = CommandSpec::new("python3", directory).args([
                directory.join("capture.py").display().to_string(),
                directory.display().to_string(),
            ]);
            let outcome = match execute(&spec) {
                Ok(status) => format!("exit {:?}", status.code()),
                Err(error) => format!("error {error}"),
            };
            fs::write(directory.join("outcome"), outcome).unwrap();
        }

        #[test]
        fn sigterm_to_rust_parent_stops_python_capture_and_owned_cli() {
            assert_signal_cleanup(libc::SIGTERM);
        }

        #[test]
        fn sighup_to_rust_parent_stops_python_capture_and_owned_cli() {
            assert_signal_cleanup(libc::SIGHUP);
        }

        #[test]
        fn sigint_to_rust_parent_stops_python_capture_and_owned_cli() {
            assert_signal_cleanup(libc::SIGINT);
        }

        #[test]
        fn capture_exit_codes_are_preserved_for_xtask_status_mapping() {
            for code in [0, 2, 3, 7] {
                let mut capture =
                    CaptureTree::with_script(&format!("import sys\nsys.exit({code})\n"));
                assert!(capture.wait_for_driver().success());
                assert_eq!(
                    fs::read_to_string(capture.directory.join("outcome")).unwrap(),
                    format!("exit Some({code})")
                );
            }
        }
    }
}
