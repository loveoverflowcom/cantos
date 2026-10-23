//! Sequential step execution shared by build, verify, preflight and ci.

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

/// Checks every prerequisite first, then runs steps in order and stops at the first failure.
/// Nothing runs when any selected step is unsupported, so a long run never ends in a surprise.
pub fn execute(
    title: &str,
    steps: &[Step],
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
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

    if missing.iter().any(|tools| !tools.is_empty()) {
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
                let names: Vec<_> = missing.iter().map(|tool| tool.label()).collect();
                let mut hints: Vec<_> = missing.iter().map(|tool| tool.hint()).collect();
                hints.dedup();
                report.record(
                    &step.name,
                    Status::Unsupported,
                    format!("missing {} ({})", names.join(", "), hints.join("; ")),
                );
            }
        }
        report.note("run `cargo xtask doctor` to inspect the local toolchain");
        return report;
    }

    let mut failed = false;
    for step in steps {
        if step.skipped {
            report.record(&step.name, Status::NotRun, "skipped by --skip");
            continue;
        }
        if failed {
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
    report
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
}
