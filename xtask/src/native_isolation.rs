//! Explicit no-hidden-infrastructure qualification of one prebuilt Rust test.
//! This adapter validates the primitive's independent controls; it never accepts a successful
//! test exit alone as isolation, native authentication, or two-process runtime evidence.

use std::{path::PathBuf, process::ExitCode, time::Duration};

use anyhow::{ensure, Result};
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::{
    capture::{Capture, SystemCapture},
    command::CommandSpec,
    report::{Report, Status},
    workspace::Workspace,
};

#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct NativeIsolationArgs {
    /// Absolute path to an already built Rust libtest executable. Never builds it.
    #[arg(long, value_name = "PATH")]
    pub test_binary: PathBuf,
    /// Exact fully qualified Rust test name; zero selected tests fail.
    #[arg(long, value_name = "NAME")]
    pub test: String,
    /// Deadline for the selected test; cleanup and controls have separate bounded budgets.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=120))]
    pub timeout_seconds: u64,
    /// Print the plan without probing tools, reading the executable, or creating resources.
    #[arg(long)]
    pub dry_run: bool,
}

fn command(workspace: &Workspace, args: &NativeIsolationArgs) -> CommandSpec {
    CommandSpec::new("python3", workspace.root()).args([
        "-I".to_owned(),
        "scripts/verify_native_isolation.py".to_owned(),
        "--test-binary".to_owned(),
        args.test_binary.to_string_lossy().into_owned(),
        "--test".to_owned(),
        args.test.clone(),
        "--timeout-seconds".to_owned(),
        args.timeout_seconds.to_string(),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ControlStatus {
    Pass,
    Fail,
    NotRun,
    Unsupported,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Controls {
    executable: ControlStatus,
    credential: ControlStatus,
    network: ControlStatus,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Source {
    sha: String,
    dirty: bool,
    diff_sha256: String,
    guard_sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    schema_version: u32,
    status: ControlStatus,
    reason: String,
    source: Option<Source>,
    binary_sha256: Option<String>,
    test_name: Option<String>,
    timeout_seconds: u64,
    tests_passed: usize,
    controls: Controls,
    network_mode: String,
    limitations: Vec<String>,
}

fn is_digest(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn outcome_status(
    output: &str,
    exit: i32,
    test_name: &str,
    timeout_seconds: u64,
) -> Result<(Outcome, Status)> {
    let outcome: Outcome = serde_json::from_str(output)?;
    ensure!(
        outcome.schema_version == 1,
        "isolation result version mismatch"
    );
    if outcome.tests_passed > 0 {
        ensure!(
            outcome.test_name.as_deref() == Some(test_name)
                && outcome.timeout_seconds == timeout_seconds,
            "selected test receipt mismatch"
        );
    }
    let status = match (outcome.status, exit) {
        (ControlStatus::Pass, 0) => {
            ensure!(
                outcome.controls.executable == ControlStatus::Pass
                    && outcome.controls.credential == ControlStatus::Pass
                    && outcome.controls.network == ControlStatus::Pass
                    && outcome.network_mode == "linux-network-namespace",
                "isolation controls incomplete"
            );
            validate_test_evidence(&outcome)?;
            Status::Pass
        }
        (ControlStatus::Fail, 1) => Status::Fail,
        (ControlStatus::Unsupported, 3) => {
            if outcome.tests_passed > 0 {
                ensure!(
                    outcome.controls.executable == ControlStatus::Pass
                        && outcome.controls.credential == ControlStatus::Pass
                        && outcome.controls.network == ControlStatus::Unsupported
                        && outcome.network_mode == "macos-best-effort",
                    "unsupported result claims inconsistent executed controls"
                );
                validate_test_evidence(&outcome)?;
            }
            Status::Unsupported
        }
        _ => anyhow::bail!("isolation status/exit mismatch"),
    };
    Ok((outcome, status))
}

fn validate_test_evidence(outcome: &Outcome) -> Result<()> {
    ensure!(
        (1..=120).contains(&outcome.timeout_seconds)
            && outcome.test_name.as_deref().is_some_and(|name| {
                !name.is_empty()
                    && name.len() <= 512
                    && name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b':')
            }),
        "selected test identity missing"
    );
    ensure!(
        outcome.tests_passed == 1,
        "exactly one passing test required"
    );
    ensure!(
        outcome
            .binary_sha256
            .as_deref()
            .is_some_and(|value| is_digest(value, 64)),
        "test executable identity missing"
    );
    let source = outcome
        .source
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("source identity missing"))?;
    ensure!(
        is_digest(&source.sha, 40)
            && is_digest(&source.diff_sha256, 64)
            && is_digest(&source.guard_sha256, 64),
        "source identity invalid"
    );
    Ok(())
}

pub fn execute(workspace: &Workspace, args: &NativeIsolationArgs) -> Result<ExitCode> {
    if args.dry_run {
        println!(
            "{}",
            serde_json::json!({
                "schema_version": 1, "status": "NOT_RUN", "dry_run": true,
                "steps": [
                    "identify one prebuilt exact Rust test and current source",
                    "create a private run directory and empty executable PATH",
                    "execute executable and credential injection negative controls",
                    "on Linux enter owned network/PID namespaces and check egress refusal and a reachable network negative control",
                    "run exactly one test with bounded output/deadline; kill only owned children before cleanup",
                    "emit a sanitized source/executable-bound receipt; macOS network isolation is UNSUPPORTED"
                ],
                "scope": "guard tooling only; no native E2E or Kani acceptance"
            })
        );
        return Ok(ExitCode::SUCCESS);
    }
    let mut report = Report::new("verify native-isolation");
    let mut capture = SystemCapture::new()?.with_cleanup_grace(Duration::from_secs(10));
    match capture.capture(&command(workspace, args), Duration::from_secs(180)) {
        Ok(captured) => match outcome_status(
            &captured.stdout,
            captured.code,
            &args.test,
            args.timeout_seconds,
        ) {
            Ok((outcome, status)) => {
                // Only the primitive's structured envelope is emitted. Test output and stderr
                // remain private even for malformed summaries, timeouts, and interruption.
                println!("{}", serde_json::to_string(&outcome)?);
                report.record(
                    "native-isolation",
                    status,
                    "see sanitized isolation receipt",
                );
            }
            Err(_) => report.record(
                "native-isolation",
                Status::Fail,
                "invalid or inconsistent isolation receipt; raw output suppressed",
            ),
        },
        Err(_) => report.record(
            "native-isolation",
            Status::Fail,
            "isolation primitive failed or exceeded its bound; raw output suppressed",
        ),
    }
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(output: &str, exit: i32) -> Result<(Outcome, Status)> {
        outcome_status(output, exit, "fixture::exact_test", 60)
    }

    fn receipt() -> serde_json::Value {
        serde_json::json!({
            "schema_version":1, "status":"PASS", "reason":"complete",
            "source":{"sha":"a".repeat(40),"dirty":false,"diff_sha256":"b".repeat(64),"guard_sha256":"c".repeat(64)},
            "binary_sha256":"d".repeat(64), "tests_passed":1,
            "test_name":"fixture::exact_test", "timeout_seconds":60,
            "controls":{"executable":"PASS","credential":"PASS","network":"PASS"},
            "network_mode":"linux-network-namespace", "limitations":[]
        })
    }

    #[test]
    fn successful_test_exit_requires_all_controls_and_exact_provenance() {
        let valid = receipt();
        assert_eq!(status(&valid.to_string(), 0).unwrap().1, Status::Pass);
        for (pointer, replacement) in [
            ("/tests_passed", serde_json::json!(0)),
            ("/tests_passed", serde_json::json!(2)),
            ("/controls/network", serde_json::json!("UNSUPPORTED")),
            ("/controls/executable", serde_json::json!("NOT_RUN")),
            ("/controls/credential", serde_json::json!("FAIL")),
            ("/network_mode", serde_json::json!("macos-best-effort")),
            ("/source/sha", serde_json::json!("unknown")),
            ("/source/diff_sha256", serde_json::json!("unknown")),
            ("/binary_sha256", serde_json::Value::Null),
            ("/test_name", serde_json::json!("fixture::different")),
            ("/timeout_seconds", serde_json::json!(61)),
        ] {
            let mut changed = valid.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            assert!(status(&changed.to_string(), 0).is_err(), "{pointer}");
        }
        assert!(status(&valid.to_string(), 1).is_err());
    }

    #[test]
    fn macos_executed_test_stays_unsupported() {
        let mut result = receipt();
        result["status"] = "UNSUPPORTED".into();
        result["controls"]["network"] = "UNSUPPORTED".into();
        result["network_mode"] = "macos-best-effort".into();
        assert_eq!(
            status(&result.to_string(), 3).unwrap().1,
            Status::Unsupported
        );
        assert!(status(&result.to_string(), 0).is_err());
    }

    #[test]
    fn lane_is_opt_in_and_plan_preserves_arguments_without_a_shell() {
        let args = NativeIsolationArgs {
            test_binary: PathBuf::from("/private/fixture path/test"),
            test: "fixture::exact_test".into(),
            timeout_seconds: 12,
            dry_run: false,
        };
        let spec = command(&Workspace::discover().unwrap(), &args);
        assert_eq!(spec.program, "python3");
        assert_eq!(spec.args[0], "-I");
        assert_eq!(spec.args[3], "/private/fixture path/test");
        assert!(
            crate::gate::verify_suite(&crate::cli::VerifySuite::NativeIsolation(args)).is_empty()
        );
        assert!(crate::gate::full()
            .iter()
            .flat_map(|gate| gate.commands(&Workspace::discover().unwrap()))
            .all(|command| !command
                .args
                .iter()
                .any(|arg| arg.contains("verify_native_isolation.py"))));
    }
}
