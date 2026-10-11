//! Outcome vocabulary and the summary every multi-step command prints.
//!
//! `PASS` ran and succeeded. `FAIL` ran (or could not start) and failed. `NOT_RUN` was selected
//! but not executed: explicitly skipped, or stopped by an earlier failure or missing
//! prerequisite. `UNSUPPORTED` cannot run here: a required tool is missing, or the target has no
//! implementation (for example a mobile surface without a Gradle project). Only `PASS` and
//! explicitly skipped `NOT_RUN` steps let a command exit successfully.

use std::{fmt, process::ExitCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    NotRun,
    Unsupported,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::NotRun => "NOT_RUN",
            Status::Unsupported => "UNSUPPORTED",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub title: String,
    pub outcomes: Vec<Outcome>,
    pub notes: Vec<String>,
}

impl Report {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            outcomes: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn record(&mut self, name: impl Into<String>, status: Status, detail: impl Into<String>) {
        self.outcomes.push(Outcome {
            name: name.into(),
            status,
            detail: detail.into(),
        });
    }

    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// At least one step passed and every other one was explicitly skipped. (A `NOT_RUN` step
    /// only exists without a `FAIL`/`UNSUPPORTED` when it was explicitly skipped.)
    pub fn succeeded(&self) -> bool {
        self.outcomes
            .iter()
            .any(|outcome| outcome.status == Status::Pass)
            && self
                .outcomes
                .iter()
                .all(|outcome| matches!(outcome.status, Status::Pass | Status::NotRun))
    }

    pub fn exit_code(&self) -> ExitCode {
        if self.succeeded() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }

    pub fn render(&self) -> String {
        let mut text = self.render_table();
        let verdict = if self.succeeded() {
            let skipped = self
                .outcomes
                .iter()
                .filter(|outcome| outcome.status == Status::NotRun)
                .count();
            if skipped == 0 {
                "PASS".to_owned()
            } else {
                format!("PASS with {skipped} explicitly skipped (NOT_RUN) step(s)")
            }
        } else if self.outcomes.iter().any(|o| o.status == Status::Fail) {
            "FAILED: a required step did not pass".to_owned()
        } else if self.outcomes.iter().all(|o| o.status == Status::NotRun) {
            "NOT_RUN: every step was skipped, so nothing was verified".to_owned()
        } else {
            "UNSUPPORTED: a required step cannot run here; nothing was reported as passing"
                .to_owned()
        };
        text.push_str(&format!("{}: {verdict}\n", self.title));
        text
    }

    /// Rows and notes without a verdict, for diagnostic listings.
    pub fn render_table(&self) -> String {
        let width = self
            .outcomes
            .iter()
            .map(|outcome| outcome.name.len())
            .max()
            .unwrap_or(0);
        let mut text = format!("\n{} summary\n", self.title);
        for outcome in &self.outcomes {
            text.push_str(&format!(
                "  {:<11}  {:<width$}  {}\n",
                outcome.status.to_string(),
                outcome.name,
                outcome.detail
            ));
        }
        for note in &self.notes {
            text.push_str(&format!("  note: {note}\n"));
        }
        text
    }

    pub fn print(&self) {
        print!("{}", self.render());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pass_and_skips_succeed() {
        let mut report = Report::new("verify");
        assert!(!report.succeeded(), "an empty report proves nothing");
        report.record("b", Status::NotRun, "skipped by --skip");
        assert!(!report.succeeded(), "skipping everything proves nothing");
        assert!(report.render().contains("verify: NOT_RUN"));
        report.record("a", Status::Pass, "");
        assert!(report.succeeded());
        assert!(report.render().contains("PASS with 1 explicitly skipped"));
        report.record("c", Status::Unsupported, "missing initdb");
        assert!(!report.succeeded());
        assert!(report.render().contains("UNSUPPORTED  c"));
        assert!(report.render().contains("verify: UNSUPPORTED"));
        report.record("d", Status::Fail, "exit status 1");
        assert!(report.render().contains("verify: FAILED"));
    }
}
