//! `seed`: explicit synthetic development data in the disposable cluster retained by
//! `verify postgres --keep`. It is never a gate, preflight selection or CI job; the PostgreSQL
//! gate already runs the same seed test as a regression without retaining its database.
//!
//! `scripts/seed_theatre.py` is the primitive: it accepts only that retained cluster, confirms
//! the port is still served by its data directory, runs the seed test through real Studio
//! flows and records the stable identities. `--reset` drops only the recorded seed database.

use std::process::ExitCode;

use anyhow::Result;
use clap::ValueEnum;

use crate::{
    build::Refusal,
    command::{CommandSpec, Runner},
    config::TEST_CLUSTER_RECORD,
    pipeline::{self, Step},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

/// Written by the primitive next to the retained cluster record; read by `serve_studio.py`.
pub const THEATRE_SEED_RECORD: &str = "target/revision-evidence/theatre-seed.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SeedTarget {
    /// Public Theatre catalog: novel, story and podcast Works published through Studio flows.
    Theatre,
}

pub fn plan(workspace: &Workspace, target: SeedTarget, reset: bool) -> Result<Step, Refusal> {
    let SeedTarget::Theatre = target;
    if !workspace.path(TEST_CLUSTER_RECORD).is_file() {
        return Err(Refusal::Usage(format!(
            "seed theatre needs the disposable cluster retained by \
             `cargo xtask verify postgres --keep` ({TEST_CLUSTER_RECORD} is missing); \
             it never uses DATABASE_URL"
        )));
    }
    let command = CommandSpec::new("python3", workspace.root())
        .args(["scripts/seed_theatre.py"])
        .arg_if(reset, "--reset");
    // Reset only drops the recorded database; seeding also builds and runs the media test.
    let tools: &[Tool] = if reset {
        &[Tool::Python3, Tool::Psql]
    } else {
        &[
            Tool::Python3,
            Tool::Psql,
            Tool::Cargo,
            Tool::Ffmpeg,
            Tool::Ffprobe,
        ]
    };
    let name = if reset {
        "seed theatre --reset (drop the recorded seed database)"
    } else {
        "seed theatre (synthetic public catalog in the retained cluster)"
    };
    Ok(Step::new(name, tools, vec![command]))
}

pub fn execute(
    workspace: &Workspace,
    target: SeedTarget,
    reset: bool,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<std::result::Result<ExitCode, Refusal>> {
    let step = match plan(workspace, target, reset) {
        Ok(step) => step,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut report = pipeline::execute("seed", &[step], tools, runner);
    if !reset && report.succeeded() {
        report.note(format!(
            "record: {THEATRE_SEED_RECORD}; serve it with `cargo xtask run cantos-theatre --web \
             --test-cluster`. Discovery data only: publication delivery and playback stay unavailable"
        ));
    }
    report.print();
    Ok(Ok(report.exit_code()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::testing::RecordingRunner, report::Status, tools::testing::FakeTools};
    use std::fs;

    /// A throwaway workspace root; removed when the guard drops.
    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn workspace_with_record(present: bool) -> (Root, Workspace) {
        let root = std::env::temp_dir().join(format!(
            "cantos-xtask-seed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        if present {
            let record = root.join(TEST_CLUSTER_RECORD);
            fs::create_dir_all(record.parent().unwrap()).unwrap();
            fs::write(record, "{}").unwrap();
        }
        let workspace = Workspace::fixture(&root).unwrap();
        (Root(root), workspace)
    }

    #[test]
    fn seed_and_reset_are_one_explicit_primitive_step_each() {
        let (_dir, workspace) = workspace_with_record(true);
        let seed = plan(&workspace, SeedTarget::Theatre, false).unwrap();
        assert_eq!(
            seed.commands[0].display(workspace.root()),
            "python3 scripts/seed_theatre.py"
        );
        assert!(seed.tools.contains(&Tool::Ffmpeg) && seed.tools.contains(&Tool::Cargo));
        let reset = plan(&workspace, SeedTarget::Theatre, true).unwrap();
        assert_eq!(
            reset.commands[0].display(workspace.root()),
            "python3 scripts/seed_theatre.py --reset"
        );
        assert_eq!(reset.tools, [Tool::Python3, Tool::Psql]);
    }

    #[test]
    fn no_retained_cluster_is_a_usage_refusal_before_any_command() {
        let (_dir, workspace) = workspace_with_record(false);
        for reset in [false, true] {
            let mut runner = RecordingRunner::default();
            let outcome = execute(
                &workspace,
                SeedTarget::Theatre,
                reset,
                &FakeTools::all(),
                &mut runner,
            )
            .unwrap();
            let Err(Refusal::Usage(message)) = outcome else {
                panic!("expected a usage refusal");
            };
            assert!(message.contains("verify postgres --keep"));
            assert!(message.contains("never uses DATABASE_URL"));
            assert!(runner.commands.is_empty());
        }
    }

    #[test]
    fn a_missing_postgres_client_is_unsupported_and_runs_nothing() {
        let (_dir, workspace) = workspace_with_record(true);
        let step = plan(&workspace, SeedTarget::Theatre, false).unwrap();
        let mut runner = RecordingRunner::default();
        let report = pipeline::execute(
            "seed",
            &[step],
            &FakeTools::without(&[Tool::Psql]),
            &mut runner,
        );
        assert!(runner.commands.is_empty());
        assert_eq!(report.outcomes[0].status, Status::Unsupported);
    }
}
