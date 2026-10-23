//! Explicit no-hidden-infrastructure qualification of one prebuilt Rust test.
//! This adapter validates the primitive's independent controls; it never accepts a successful
//! test exit alone as isolation, native authentication, or two-process runtime evidence.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use anyhow::{ensure, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    /// Absolute path to an already built `cantos-identity` executable the test starts as a child.
    /// It reaches the test only as `CANTOS_TEST_IDENTITY_BINARY` and is digest-bound before and
    /// after the run. Never builds it.
    #[arg(long, value_name = "PATH")]
    pub identity_binary: Option<PathBuf>,
    /// Print the plan without probing tools, reading the executable, or creating resources.
    #[arg(long)]
    pub dry_run: bool,
}

/// The receipt schema of the primitive; version 2 added the Identity child digest.
const SCHEMA_VERSION: u32 = 2;

pub(crate) fn command(workspace: &Workspace, args: &NativeIsolationArgs) -> CommandSpec {
    let mut command = CommandSpec::new("python3", workspace.root()).args([
        "-I".to_owned(),
        "scripts/verify_native_isolation.py".to_owned(),
        "--test-binary".to_owned(),
        args.test_binary.to_string_lossy().into_owned(),
        "--test".to_owned(),
        args.test.clone(),
        "--timeout-seconds".to_owned(),
        args.timeout_seconds.to_string(),
    ]);
    if let Some(identity) = &args.identity_binary {
        command = command.args([
            "--identity-binary".to_owned(),
            identity.to_string_lossy().into_owned(),
        ]);
    }
    command
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
pub(crate) struct Source {
    pub(crate) sha: String,
    pub(crate) dirty: bool,
    pub(crate) diff_sha256: String,
    guard_sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Outcome {
    schema_version: u32,
    status: ControlStatus,
    reason: String,
    source: Option<Source>,
    binary_sha256: Option<String>,
    identity_binary_sha256: Option<String>,
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

impl Outcome {
    pub(crate) fn source(&self) -> Option<&Source> {
        self.source.as_ref()
    }
    pub(crate) fn binary_sha256(&self) -> Option<&str> {
        self.binary_sha256.as_deref()
    }
    pub(crate) fn identity_binary_sha256(&self) -> Option<&str> {
        self.identity_binary_sha256.as_deref()
    }
    pub(crate) fn tests_passed(&self) -> usize {
        self.tests_passed
    }
    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }
}

/// Validate one primitive receipt against the invocation that produced it. `identity` says
/// whether a child executable was supplied: its digest must then be present, and absent otherwise.
pub(crate) fn outcome_status(
    output: &str,
    exit: i32,
    test_name: &str,
    timeout_seconds: u64,
    identity: bool,
) -> Result<(Outcome, Status)> {
    let outcome: Outcome = serde_json::from_str(output)?;
    ensure!(
        outcome.schema_version == SCHEMA_VERSION,
        "isolation result version mismatch"
    );
    if !identity {
        ensure!(
            outcome.identity_binary_sha256.is_none(),
            "receipt binds a child that was not supplied"
        );
    }
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
            validate_test_evidence(&outcome, identity)?;
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
                validate_test_evidence(&outcome, identity)?;
            }
            Status::Unsupported
        }
        _ => anyhow::bail!("isolation status/exit mismatch"),
    };
    Ok((outcome, status))
}

fn validate_test_evidence(outcome: &Outcome, identity: bool) -> Result<()> {
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
    if identity {
        ensure!(
            outcome
                .identity_binary_sha256
                .as_deref()
                .is_some_and(|value| is_digest(value, 64))
                && outcome.identity_binary_sha256 != outcome.binary_sha256,
            "identity child executable identity missing"
        );
    }
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

fn sha256(path: &Path) -> Option<String> {
    std::fs::read(path)
        .ok()
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
}

/// A receipt for a test that ran must name the digests of exactly the files this invocation
/// supplied, hashed again here after the run rather than trusted from the primitive.
fn binds_supplied(outcome: &Outcome, test_binary: &Path, identity: Option<&Path>) -> bool {
    outcome.tests_passed == 0
        || (outcome.binary_sha256.is_some()
            && outcome.binary_sha256 == sha256(test_binary)
            && outcome.identity_binary_sha256 == identity.and_then(sha256))
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
                    "with --identity-binary: pass that one absolute path as CANTOS_TEST_IDENTITY_BINARY and bind its digest before and after",
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
            args.identity_binary.is_some(),
        ) {
            Ok((outcome, status)) => {
                // Only the primitive's structured envelope is emitted. Test output and stderr
                // remain private even for malformed summaries, timeouts, and interruption.
                println!("{}", serde_json::to_string(&outcome)?);
                if binds_supplied(&outcome, &args.test_binary, args.identity_binary.as_deref()) {
                    report.record(
                        "native-isolation",
                        status,
                        "see sanitized isolation receipt",
                    );
                } else {
                    report.record(
                        "native-isolation",
                        Status::Fail,
                        "receipt does not bind the supplied executables",
                    );
                }
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
        outcome_status(output, exit, "fixture::exact_test", 60, false)
    }

    fn receipt() -> serde_json::Value {
        serde_json::json!({
            "schema_version":2, "status":"PASS", "reason":"complete",
            "source":{"sha":"a".repeat(40),"dirty":false,"diff_sha256":"b".repeat(64),"guard_sha256":"c".repeat(64)},
            "binary_sha256":"d".repeat(64), "identity_binary_sha256":null, "tests_passed":1,
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
            ("/schema_version", serde_json::json!(1)),
            // A child digest without a supplied child is a receipt for another invocation.
            ("/identity_binary_sha256", serde_json::json!("e".repeat(64))),
        ] {
            let mut changed = valid.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            assert!(status(&changed.to_string(), 0).is_err(), "{pointer}");
        }
        assert!(status(&valid.to_string(), 1).is_err());
    }

    #[test]
    fn a_supplied_child_must_be_bound_by_its_own_valid_digest() {
        let child = |receipt: &serde_json::Value, exit: i32| {
            outcome_status(&receipt.to_string(), exit, "fixture::exact_test", 60, true)
        };
        let mut valid = receipt();
        valid["identity_binary_sha256"] = "e".repeat(64).into();
        let (outcome, status) = child(&valid, 0).unwrap();
        assert_eq!(status, Status::Pass);
        assert_eq!(
            outcome.identity_binary_sha256(),
            Some("e".repeat(64).as_str())
        );
        for replacement in [
            serde_json::Value::Null,
            serde_json::json!("unknown"),
            serde_json::json!("E".repeat(64)),
            // The test executable's digest cannot stand in for the child's.
            serde_json::json!("d".repeat(64)),
        ] {
            let mut changed = valid.clone();
            changed["identity_binary_sha256"] = replacement.clone();
            assert!(child(&changed, 0).is_err(), "{replacement}");
        }
        // A failed run with a supplied child stays FAIL; it is never promoted by the digest.
        let mut failed = valid.clone();
        failed["status"] = "FAIL".into();
        failed["tests_passed"] = 0.into();
        assert_eq!(child(&failed, 1).unwrap().1, Status::Fail);
        let mut unsupported = valid;
        unsupported["status"] = "UNSUPPORTED".into();
        unsupported["controls"]["network"] = "UNSUPPORTED".into();
        unsupported["network_mode"] = "macos-best-effort".into();
        assert_eq!(child(&unsupported, 3).unwrap().1, Status::Unsupported);
        unsupported["identity_binary_sha256"] = serde_json::Value::Null;
        assert!(child(&unsupported, 3).is_err());
    }

    #[test]
    fn a_receipt_binds_the_supplied_files_as_hashed_after_the_run() {
        let directory = std::env::temp_dir().join(format!("cantos-bind-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let test = directory.join("test");
        let child = directory.join("child");
        std::fs::write(&test, b"test executable").unwrap();
        std::fs::write(&child, b"identity executable").unwrap();
        let mut value = receipt();
        value["binary_sha256"] = sha256(&test).unwrap().into();
        value["identity_binary_sha256"] = sha256(&child).unwrap().into();
        let outcome: Outcome = serde_json::from_value(value.clone()).unwrap();
        assert!(binds_supplied(&outcome, &test, Some(&child)));
        // A child that is not supplied, or a file replaced after the run, never binds.
        assert!(!binds_supplied(&outcome, &test, None));
        std::fs::write(&child, b"replaced identity executable").unwrap();
        assert!(!binds_supplied(&outcome, &test, Some(&child)));
        std::fs::write(&test, b"replaced test executable").unwrap();
        value["identity_binary_sha256"] = serde_json::Value::Null;
        let outcome: Outcome = serde_json::from_value(value.clone()).unwrap();
        assert!(!binds_supplied(&outcome, &test, None));
        // A run that executed nothing has nothing to bind.
        value["tests_passed"] = 0.into();
        let outcome: Outcome = serde_json::from_value(value).unwrap();
        assert!(binds_supplied(&outcome, &test, None));
        std::fs::remove_dir_all(&directory).unwrap();
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
            identity_binary: None,
            dry_run: false,
        };
        let spec = command(&Workspace::discover().unwrap(), &args);
        assert_eq!(spec.program, "python3");
        assert_eq!(spec.args[0], "-I");
        assert_eq!(spec.args[3], "/private/fixture path/test");
        assert!(!spec.args.iter().any(|arg| arg == "--identity-binary"));
        let child = NativeIsolationArgs {
            identity_binary: Some(PathBuf::from("/private/fixture path/cantos-identity")),
            ..args.clone()
        };
        let spec = command(&Workspace::discover().unwrap(), &child);
        assert_eq!(
            spec.args[spec.args.len() - 2..],
            [
                "--identity-binary".to_owned(),
                "/private/fixture path/cantos-identity".to_owned()
            ]
        );
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
