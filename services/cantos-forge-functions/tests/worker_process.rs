//! Local process/stdio evidence only: no Node runtime, deployment, provider or authorization.

use std::{
    io::Write,
    process::{Command, Stdio},
};

fn invoke(bytes: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cantos-forge-worker"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn actual_worker_exits_after_one_bounded_capabilities_reply() {
    let output = invoke(br#"{"schema_version":"cantos-forge.worker/v1","request_id":"process-01","operation":{"kind":"capabilities"}}"#);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stderr, b"");
    let reply: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        reply,
        serde_json::json!({"schema_version":"cantos-forge.worker/v1","request_id":"process-01","result":{"kind":"capabilities","operations":["capabilities","check_text"],"generation":"unavailable"}})
    );
    assert_eq!(output.stdout.last(), Some(&b'\n'));
}

#[test]
fn actual_worker_rejects_process_configuration_without_reflecting_it() {
    let output = invoke(br#"{"schema_version":"cantos-forge.worker/v1","request_id":"process-01","operation":{"kind":"capabilities"},"env":{"TOKEN":"private-secret"}}"#);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, b"");
    let reply: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        reply["result"],
        serde_json::json!({"kind":"rejected","code":"invalid_request"})
    );
    assert!(!String::from_utf8(output.stdout)
        .unwrap()
        .contains("private-secret"));
}
