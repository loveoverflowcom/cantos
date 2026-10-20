//! The recorded capability expectations hold on this host, for the engine in-process and for
//! the process-level probes (SIGKILL, second owner, file-size limit) through the report binary.

use std::process::Command;

use cantos_store_turso::capability;
use cantos_store_turso::testing::TestRun;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_engine_probe_matches_its_recorded_expectation() {
    let run = TestRun::new("probes");
    let report = capability::run_all(run.dir()).await;
    let unexpected: Vec<_> = report
        .unexpected()
        .iter()
        .map(|probe| format!("{}: {} ({})", probe.id, probe.outcome.label(), probe.detail))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{}\n{}",
        unexpected.join("\n"),
        report.render()
    );
    // Non-vacuity: the three outcomes all occur, so a probe that always passed would show.
    for outcome in [
        capability::Outcome::Pass,
        capability::Outcome::Unsupported,
        capability::Outcome::Fail,
    ] {
        assert!(
            report.probes.iter().any(|probe| probe.outcome == outcome),
            "no probe reported {outcome:?}"
        );
    }
}

#[test]
fn the_report_binary_exits_cleanly_and_includes_the_process_probes() {
    let output = Command::new(env!("CARGO_BIN_EXE_cantos-turso-capability"))
        .arg("report")
        .output()
        .expect("the report binary starts");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{text}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for id in [
        "process.kill9_after_ack",
        "process.second_owner_refused",
        "process.file_size_limit",
    ] {
        assert!(text.contains(id), "missing {id}\n{text}");
    }
}
