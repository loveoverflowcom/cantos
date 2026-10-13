//! `cargo xtask doctor`: what this machine can build, run and verify. Read-only; it installs
//! nothing and never contacts a provider.

use std::process::ExitCode;

use crate::{
    config::{REQUIRED_RUST, THEATRE_MOBILE_DIR, TRUNK_VERSION},
    gate::{self, Gate},
    report::{Report, Status},
    tools::{SystemTools, Tool, ToolProbe},
};

/// Tools without which no xtask command is useful.
const CORE: [Tool; 3] = [Tool::Cargo, Tool::Git, Tool::Python3];

pub fn run(tools: &SystemTools) -> ExitCode {
    println!("Cantos developer environment");
    match tools.rustc_version() {
        Some(version) => {
            let pinned = version.starts_with(&format!("rustc {REQUIRED_RUST}."));
            println!("  {version}");
            if !pinned {
                println!(
                    "  note: packages declare rust-version {REQUIRED_RUST} and CI pins 1.87.0; \
                     select it with `rustup override set 1.87.0` or RUSTUP_TOOLCHAIN=1.87.0"
                );
            }
        }
        None => println!("  rustc: not found"),
    }

    let mut report = Report::new("doctor");
    for tool in Tool::ALL {
        if tools.available(tool) {
            let version = tools.version(tool).unwrap_or_default();
            let advice = if tool == Tool::Trunk && !version.contains(TRUNK_VERSION) {
                format!("{version} (documented: {TRUNK_VERSION})")
            } else {
                version
            };
            report.record(format!("tool {}", tool.label()), Status::Pass, advice);
        } else {
            report.record(
                format!("tool {}", tool.label()),
                Status::Unsupported,
                tool.hint(),
            );
        }
    }

    let mut gates = gate::full();
    gates.push(Gate::ContractTests);
    gates.extend(crate::kani::Kernel::ALL.into_iter().map(Gate::Kani));
    gates.sort();
    for gate in gates {
        let missing = tools.missing(&gate.tools());
        if missing.is_empty() {
            report.record(
                format!("gate {}", gate.name()),
                Status::Pass,
                "runnable here",
            );
        } else {
            let names: Vec<_> = missing.iter().map(|tool| tool.label()).collect();
            report.record(
                format!("gate {}", gate.name()),
                Status::Unsupported,
                format!("missing {}", names.join(", ")),
            );
        }
    }
    report.record(
        "surface cantos-theatre --mobile",
        Status::Unsupported,
        format!("production native listening is unavailable; {THEATRE_MOBILE_DIR} has a separate theatre-demo mock UI"),
    );
    report.record(
        "services/kanidm",
        Status::NotRun,
        "configuration only; xtask neither builds nor starts an identity provider",
    );
    print!("{}", report.render_table());

    let core_missing = tools.missing(&CORE);
    if core_missing.is_empty() {
        println!(
            "doctor: UNSUPPORTED rows describe this machine or reserved surfaces; \
             commands that need them refuse to run instead of passing."
        );
        ExitCode::SUCCESS
    } else {
        println!("doctor: core tools are missing; xtask cannot verify this repository here.");
        ExitCode::FAILURE
    }
}
