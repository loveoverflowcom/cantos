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
    let selection = select::select_with_kani(changed.as_deref(), &graph, options.kani);
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
