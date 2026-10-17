//! Opt-in local registration fixtures. Planning performs no tool probes or resource writes.

use std::{process::ExitCode, time::Duration};

use anyhow::Result;
use clap::ValueEnum;
use serde::Deserialize;

use crate::{
    capture::{Capture, SystemCapture},
    command::CommandSpec,
    report::{Report, Status},
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Suite {
    FirebaseEmulator,
    FirebaseWebE2e,
}

impl Suite {
    pub fn name(self) -> &'static str {
        match self {
            Self::FirebaseEmulator => "firebase-emulator",
            Self::FirebaseWebE2e => "firebase-web-e2e",
        }
    }
}

fn command(workspace: &Workspace, suite: Suite) -> CommandSpec {
    CommandSpec::new("python3", workspace.root()).args([
        "-I",
        "scripts/test_theatre_auth.py",
        "--suite",
        suite.name(),
    ])
}

/// No source identity, subprocess, environment probe or receipt is resolved here.
fn plan(suite: Suite) -> serde_json::Value {
    let steps = match suite {
        Suite::FirebaseEmulator => vec![
            "check selected prerequisites without installing tools or reading cloud credentials",
            "enter owned Linux user/network/PID namespaces; enable only the loopback interface",
            "assert forbidden destinations cannot connect; allocate a fresh private fixture directory",
            "initialize a disposable PostgreSQL cluster with the existing migration/grant fixture",
            "run exactly the selected ignored Rust acceptance test; require one passing test",
            "stop and reap owned fixtures; emit a redacted machine-readable result",
        ],
        Suite::FirebaseWebE2e => vec![
            "report UNSUPPORTED: the real disposable Kanidm credential-setup/OIDC host fixture is not implemented",
            "start no emulator, database, browser or issuer; report zero selected tests",
        ],
    };
    serde_json::json!({
        "schema_version": 1,
        "suite": suite.name(),
        "status": "NOT_RUN",
        "dry_run": true,
        "steps": steps,
        "firebase_contract": "installed firebase-tools 14.2.2 Auth server.js; explicit CANTOS_TEST_FIREBASE_AUTH_MODULE",
        "web_e2e": "UNSUPPORTED until the real disposable Kanidm credential-setup/OIDC host fixture is implemented",
        "ordinary_gates": "not selected by build, preflight, ci, deploy or other verify suites"
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    schema_version: u32,
    suite: String,
    status: String,
    reason: String,
    source: serde_json::Value,
    versions: serde_json::Value,
    topology: serde_json::Value,
    scenarios: Vec<String>,
    tests_passed: usize,
    layers: serde_json::Value,
}

fn outcome_status(output: &str, exit: i32, suite: Suite) -> Result<(Outcome, Status)> {
    let outcome: Outcome = serde_json::from_str(output)?;
    anyhow::ensure!(
        outcome.schema_version == 1 && outcome.suite == suite.name(),
        "fixture result identity mismatch"
    );
    let status = match (outcome.status.as_str(), exit) {
        ("PASS", 0) if outcome.tests_passed > 0 && !outcome.scenarios.is_empty() => Status::Pass,
        ("FAIL", 1) => Status::Fail,
        ("UNSUPPORTED", 3) => Status::Unsupported,
        _ => anyhow::bail!("fixture returned inconsistent status or selected zero tests"),
    };
    Ok((outcome, status))
}

pub fn execute(workspace: &Workspace, suite: Suite, dry_run: bool) -> Result<ExitCode> {
    if dry_run {
        println!("{}", serde_json::to_string(&plan(suite))?);
        return Ok(ExitCode::SUCCESS);
    }
    let mut report = Report::new("verify theatre-auth");
    let mut capture = SystemCapture::new()?.with_cleanup_grace(Duration::from_secs(10));
    match capture.capture(&command(workspace, suite), Duration::from_secs(1800)) {
        Ok(captured) => match outcome_status(&captured.stdout, captured.code, suite) {
            Ok((outcome, status)) => {
                report.record(suite.name(), status, &outcome.reason);
                // Re-serialize the allowlisted envelope. Child stderr and raw test output
                // are deliberately never echoed, including on malformed-result failures.
                println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": outcome.schema_version,
                        "suite": outcome.suite, "status": outcome.status,
                        "reason": outcome.reason, "source": outcome.source,
                        "versions": outcome.versions, "topology": outcome.topology,
                        "scenarios": outcome.scenarios, "tests_passed": outcome.tests_passed,
                        "layers": outcome.layers,
                    })
                );
            }
            Err(_) => report.record(
                suite.name(),
                Status::Fail,
                "invalid or inconsistent fixture summary; raw output suppressed",
            ),
        },
        Err(error) => report.record(suite.name(), Status::Fail, error.to_string()),
    }
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_is_pure_and_never_claims_an_executed_test() {
        for suite in [Suite::FirebaseEmulator, Suite::FirebaseWebE2e] {
            let planned = plan(suite);
            assert_eq!(planned["status"], "NOT_RUN");
            assert_eq!(planned["dry_run"], true);
            assert_eq!(planned["suite"], suite.name());
            let spec = command(&Workspace::discover().unwrap(), suite);
            assert_eq!(spec.program, "python3");
            assert_eq!(spec.args[0], "-I");
            assert!(!spec.args.iter().any(|arg| arg.contains("npx")));
            assert!(
                crate::gate::verify_suite(&crate::cli::VerifySuite::TheatreAuth {
                    suite,
                    dry_run: false
                })
                .is_empty()
            );
        }
        let workspace = Workspace::discover().unwrap();
        assert!(crate::gate::full()
            .iter()
            .flat_map(|gate| gate.commands(&workspace))
            .all(|command| !command
                .args
                .iter()
                .any(|arg| arg.contains("test_theatre_auth"))));
    }

    fn envelope(status: &str, count: usize) -> String {
        serde_json::json!({"schema_version":1,"suite":"firebase-emulator","status":status,
            "reason":"fixture", "source":null,"versions":{},"topology":{},
            "scenarios":["registration-http"],"tests_passed":count,"layers":{}})
        .to_string()
    }

    #[test]
    fn zero_selected_tests_and_status_exit_mismatches_never_pass() {
        assert!(outcome_status(&envelope("PASS", 0), 0, Suite::FirebaseEmulator).is_err());
        assert!(outcome_status(&envelope("PASS", 1), 1, Suite::FirebaseEmulator).is_err());
        assert!(outcome_status(&envelope("PASS", 1), 0, Suite::FirebaseWebE2e).is_err());
        assert_eq!(
            outcome_status(&envelope("UNSUPPORTED", 0), 3, Suite::FirebaseEmulator)
                .unwrap()
                .1,
            Status::Unsupported
        );
    }
}
