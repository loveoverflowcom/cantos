//! `cargo xtask preflight`: the developer safety gate before pushing.

mod changes;
mod graph;
mod select;

use std::{collections::BTreeSet, process::ExitCode};

use anyhow::Result;

use crate::{
    command::Runner,
    gate::{self, Gate, GateKind},
    kani, pipeline,
    tools::ToolProbe,
    workspace::Workspace,
};

pub struct PreflightOptions {
    pub kani: bool,
    pub base: String,
    pub changed_only: bool,
    pub skip: BTreeSet<GateKind>,
    pub dry_run: bool,
}

pub fn run(
    workspace: &Workspace,
    options: &PreflightOptions,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    println!("Cantos preflight");
    let changed = if options.changed_only {
        let paths = changes::changed_paths(workspace.root(), &options.base)?;
        println!("  base:          {}", options.base);
        println!("  changed paths: {}", paths.len());
        Some(paths)
    } else {
        None
    };
    let graph = match changed {
        Some(_) => graph::PackageGraph::load(workspace.root())?,
        None => graph::PackageGraph::default(),
    };
    let mut selection = select::select_with_kani(changed.as_deref(), &graph, options.kani);
    for gate in &mut selection.gates {
        if let Gate::Repository { base } = gate {
            *base = Some(options.base.clone());
        }
    }
    match &selection.broadened {
        Some(reason) => println!("  impact:        every gate ({reason})"),
        None if selection.affected.is_empty() => println!("  impact:        no Cargo package"),
        None => println!(
            "  impact:        {}",
            selection
                .affected
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }

    let mut steps = gate::steps(&selection.gates, workspace);
    for (step, gate) in steps.iter_mut().zip(&selection.gates) {
        step.skipped = options.skip.contains(&gate.kind());
    }
    println!("  plan:");
    pipeline::print_plan(&steps, workspace.root());
    for note in &selection.notes {
        println!("  note: {note}");
    }
    if options.dry_run {
        println!("\nDry run: nothing was executed (every gate is NOT_RUN).");
        return Ok(ExitCode::SUCCESS);
    }

    if selection
        .gates
        .iter()
        .any(|gate| matches!(gate, Gate::Kani(_)))
        && !options.skip.contains(&GateKind::Kani)
    {
        if let Err(error) = kani::prepare_configuration(workspace, &mut steps) {
            return Ok(kani::configuration_refusal("preflight", &steps, error));
        }
    }
    let mut report = pipeline::execute("preflight", &steps, tools, runner);
    for note in &selection.notes {
        report.note(note.clone());
    }
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::testing::RecordingRunner, tools::testing::FakeTools};
    use std::{fs, process::Command, time::SystemTime};

    #[test]
    fn repository_localization_uses_the_preflight_base_in_both_modes() {
        let root = std::env::temp_dir().join(format!(
            "xtask-preflight-base-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("xtask/src")).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=['xtask']\n").unwrap();
        fs::write(
            root.join("xtask/Cargo.toml"),
            "[package]\nname='fixture'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        fs::write(root.join("xtask/src/main.rs"), "fn main() {}\n").unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec!["commit", "-qm", "base"],
            vec!["tag", "integration-base"],
        ] {
            let output = Command::new("git")
                .args([
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                ])
                .args(args)
                .current_dir(&root)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        }
        fs::write(root.join("README.md"), "Changed documentation.\n").unwrap();
        let workspace = Workspace::fixture(&root).unwrap();
        for changed_only in [false, true] {
            let options = PreflightOptions {
                kani: false,
                base: "integration-base".into(),
                changed_only,
                skip: BTreeSet::new(),
                dry_run: false,
            };
            let mut runner = RecordingRunner::default();
            assert_eq!(
                run(&workspace, &options, &FakeTools::all(), &mut runner).unwrap(),
                ExitCode::SUCCESS
            );
            let check = runner
                .commands
                .iter()
                .find(|command| {
                    command
                        .args
                        .first()
                        .is_some_and(|arg| arg == "scripts/check_repository.py")
                })
                .unwrap();
            assert_eq!(
                check.args,
                ["scripts/check_repository.py", "--base", "integration-base"],
                "changed_only={changed_only}"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
