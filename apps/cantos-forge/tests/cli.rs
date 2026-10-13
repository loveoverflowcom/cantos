//! Runs the built `cantos-forge` binary: exit codes and the stdout/stderr split.

use std::process::{Command, Output};

fn forge(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cantos-forge"))
        .args(args)
        .output()
        .expect("the cantos-forge binary starts")
}

#[test]
fn capabilities_json_exits_0_with_one_envelope_on_stdout() {
    let output = forge(&["capabilities", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert_eq!(stdout.lines().count(), 1);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).expect("JSON envelope");
    assert_eq!(envelope["schema"], "cantos-forge.cli/v1");
    assert_eq!(envelope["outcome"], "ok");
    assert_eq!(
        envelope["data"]["commands"],
        serde_json::json!(["capabilities"])
    );
    assert_eq!(envelope["data"]["handoff"], "unsupported");
}

#[test]
fn an_unknown_command_exits_2_with_a_diagnostic_on_stderr_only() {
    let output = forge(&["run", "plan-1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand 'run'"));
}
