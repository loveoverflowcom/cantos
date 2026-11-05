//! Step execution shared by build, run, release, verify, preflight and ci.
//!
//! Two modes share one outcome vocabulary. [`execute`] is an ordered pipeline whose later steps
//! may consume earlier results (build, run, release): one missing tool means nothing runs and the
//! first failure stops the rest. [`execute_gates`] runs independent verification gates (`verify`,
//! `preflight`, `ci`): a missing tool or a failure affects only its own gate, so one run reports
//! every independent result. Both fail the command on any `FAIL` or `UNSUPPORTED`.

use std::path::Path;

use crate::{
    command::{CommandSpec, Runner},
    report::{Report, Status},
    tools::{Tool, ToolProbe},
};

/// A named unit of work: its prerequisites and the commands that realize it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub tools: Vec<Tool>,
    pub commands: Vec<CommandSpec>,
    /// Set when the user explicitly excluded this step; it is reported as `NOT_RUN`.
    pub skipped: bool,
}

impl Step {
    pub fn new(name: impl Into<String>, tools: &[Tool], commands: Vec<CommandSpec>) -> Self {
        Self {
            name: name.into(),
            tools: tools.to_vec(),
            commands,
            skipped: false,
        }
    }
}

/// Prints the plan without running it.
pub fn print_plan(steps: &[Step], root: &Path) {
    for step in steps {
        let suffix = if step.skipped { "  [skipped]" } else { "" };
        println!("  - {}{suffix}", step.name);
        for command in &step.commands {
            println!("      $ {}", command.display(root));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Later steps may depend on earlier ones.
    Sequence,
    /// Every step is a self-contained gate.
    Independent,
}

/// Checks every prerequisite first, then runs steps in order and stops at the first failure.
/// Nothing runs when any selected step is unsupported, so a long run never ends in a surprise.
pub fn execute(
    title: &str,
    steps: &[Step],
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Report {
    run(title, steps, tools, runner, Mode::Sequence)
}

/// Runs independent verification gates. Unsupported gates are announced before anything runs,
/// then every other selected gate runs even after one fails. Commands inside one gate still stop
/// at their first failure.
pub fn execute_gates(
    title: &str,
    steps: &[Step],
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Report {
    run(title, steps, tools, runner, Mode::Independent)
}

fn run(
    title: &str,
    steps: &[Step],
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
    mode: Mode,
) -> Report {
    let mut report = Report::new(title);
    let missing: Vec<Vec<Tool>> = steps
        .iter()
        .map(|step| {
            if step.skipped {
                Vec::new()
            } else {
                tools.missing(&step.tools)
            }
        })
        .collect();
    let unsupported: Vec<&str> = steps
        .iter()
        .zip(&missing)
        .filter(|(_, missing)| !missing.is_empty())
        .map(|(step, _)| step.name.as_str())
        .collect();

    if !unsupported.is_empty() && mode == Mode::Sequence {
        for (step, missing) in steps.iter().zip(&missing) {
            if step.skipped {
                report.record(&step.name, Status::NotRun, "skipped by --skip");
            } else if missing.is_empty() {
                report.record(
                    &step.name,
                    Status::NotRun,
                    "not started: another selected step is unsupported here",
                );
            } else {
                report.record(&step.name, Status::Unsupported, missing_detail(missing));
            }
        }
        report.note("run `cargo xtask doctor` to inspect the local toolchain");
        return report;
    }
    if !unsupported.is_empty() {
        println!(
            "\nUNSUPPORTED here: {}. The other selected gates still run; the result stays UNSUPPORTED.",
            unsupported.join(", ")
        );
    }

    let mut failed = false;
    for (step, missing) in steps.iter().zip(&missing) {
        if step.skipped {
            report.record(&step.name, Status::NotRun, "skipped by --skip");
            continue;
        }
        if !missing.is_empty() {
            report.record(&step.name, Status::Unsupported, missing_detail(missing));
            continue;
        }
        if failed && mode == Mode::Sequence {
            report.record(
                &step.name,
                Status::NotRun,
                "not started: an earlier step failed",
            );
            continue;
        }
        println!("\n==> {}", step.name);
        match step
            .commands
            .iter()
            .try_for_each(|command| runner.run(command))
        {
            Ok(()) => report.record(&step.name, Status::Pass, ""),
            Err(error) => {
                failed = true;
                report.record(&step.name, Status::Fail, format!("{error:#}"));
            }
        }
    }
    if !unsupported.is_empty() {
        report.note("run `cargo xtask doctor` to inspect the local toolchain");
    }
    report
}

fn missing_detail(missing: &[Tool]) -> String {
    let names: Vec<_> = missing.iter().map(|tool| tool.label()).collect();
    let mut hints: Vec<_> = missing.iter().map(|tool| tool.hint()).collect();
    hints.dedup();
    format!("missing {} ({})", names.join(", "), hints.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::testing::RecordingRunner, tools::testing::FakeTools};

    fn step(name: &str, tools: &[Tool], program: &str) -> Step {
        Step::new(name, tools, vec![CommandSpec::new(program, "/repo")])
    }

    #[test]
    fn runs_in_order_and_stops_after_a_failure() {
        let steps = [
            step("one", &[Tool::Cargo], "first"),
            step("two", &[Tool::Cargo], "second"),
            step("three", &[Tool::Cargo], "third"),
        ];
        let mut runner = RecordingRunner {
            fail_on: Some("second".into()),
            ..Default::default()
        };
        let report = execute("test", &steps, &FakeTools::all(), &mut runner);
        let statuses: Vec<_> = report.outcomes.iter().map(|o| o.status).collect();
        assert_eq!(statuses, [Status::Pass, Status::Fail, Status::NotRun]);
        assert_eq!(runner.commands.len(), 2);
        assert!(!report.succeeded());
    }

    #[test]
    fn missing_tools_make_the_step_unsupported_and_nothing_runs() {
        let mut skipped = step("skipped", &[Tool::Gradle], "gradle");
        skipped.skipped = true;
        let steps = [
            step("cheap", &[Tool::Python3], "python3"),
            step("web", &[Tool::Trunk], "trunk"),
            skipped,
        ];
        let mut runner = RecordingRunner::default();
        let report = execute(
            "test",
            &steps,
            &FakeTools::without(&[Tool::Trunk, Tool::Gradle]),
            &mut runner,
        );
        assert!(runner.commands.is_empty());
        let statuses: Vec<_> = report.outcomes.iter().map(|o| o.status).collect();
        assert_eq!(
            statuses,
            [Status::NotRun, Status::Unsupported, Status::NotRun]
        );
        assert!(report.outcomes[1].detail.contains("trunk"));
        assert!(!report.succeeded());
    }

    #[test]
    fn explicit_skips_are_reported_but_do_not_fail() {
        let mut skipped = step("gradle-preview", &[Tool::Gradle], "python3");
        skipped.skipped = true;
        let steps = [step("rust", &[Tool::Cargo], "cargo"), skipped];
        let mut runner = RecordingRunner::default();
        let report = execute(
            "test",
            &steps,
            &FakeTools::without(&[Tool::Gradle]),
            &mut runner,
        );
        assert_eq!(runner.commands.len(), 1);
        assert_eq!(report.outcomes[1].status, Status::NotRun);
        assert!(report.succeeded());
    }

    #[test]
    fn independent_gates_keep_running_after_a_failure() {
        let steps = [
            step("format", &[Tool::Cargo], "fmt"),
            step("lint", &[Tool::Cargo], "clippy"),
            step("tests", &[Tool::Cargo], "test"),
        ];
        let mut runner = RecordingRunner {
            fail_on: Some("fmt".into()),
            ..Default::default()
        };
        let report = execute_gates("test", &steps, &FakeTools::all(), &mut runner);
        let statuses: Vec<_> = report.outcomes.iter().map(|o| o.status).collect();
        assert_eq!(statuses, [Status::Fail, Status::Pass, Status::Pass]);
        assert_eq!(runner.commands.len(), 3);
        assert!(
            !report.succeeded(),
            "a FAIL is never hidden by later passes"
        );
    }

    #[test]
    fn a_missing_tool_blocks_only_its_own_gate() {
        let mut skipped = step("skipped", &[Tool::Gradle], "gradle");
        skipped.skipped = true;
        let steps = [
            step("rust", &[Tool::Cargo], "cargo"),
            step("web", &[Tool::Trunk], "trunk"),
            skipped,
            step("repository", &[Tool::Python3], "python3"),
        ];
        let mut runner = RecordingRunner::default();
        let report = execute_gates(
            "test",
            &steps,
            &FakeTools::without(&[Tool::Trunk, Tool::Gradle]),
            &mut runner,
        );
        let programs: Vec<_> = runner.commands.iter().map(|c| c.program.as_str()).collect();
        assert_eq!(programs, ["cargo", "python3"]);
        let statuses: Vec<_> = report.outcomes.iter().map(|o| o.status).collect();
        assert_eq!(
            statuses,
            [
                Status::Pass,
                Status::Unsupported,
                Status::NotRun,
                Status::Pass
            ]
        );
        assert!(report.outcomes[1].detail.contains("trunk"));
        assert!(!report.succeeded(), "UNSUPPORTED still fails the command");
        assert!(report.render().contains("test: UNSUPPORTED"));
    }
}
