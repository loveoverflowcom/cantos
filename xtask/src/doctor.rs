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
            let pinned = is_pinned_rustc(&version);
            println!("  {version}");
            if !pinned {
                println!(
                    "  note: packages and rust-toolchain.toml pin {REQUIRED_RUST}; \
                     remove any conflicting override or select RUSTUP_TOOLCHAIN={REQUIRED_RUST}"
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
    gates.push(Gate::Behavior {
        capability: None,
        setup: false,
        conformance: false,
    });
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
        format!("production native listening is unavailable; {THEATRE_MOBILE_DIR} has a JVM-only Desktop Preview (theatre-preview) over mock ports"),
    );
    report.record(
        "native composition (cantos-server + cantos-identity)",
        if cfg!(unix) {
            Status::Pass
        } else {
            Status::Unsupported
        },
        if cfg!(unix) {
            "run/seed own two native processes over private Turso files and a Unix socket; no database server, container or identity provider is needed"
        } else {
            "Identity's private RPC is a Unix socket; run/seed need a Unix host"
        },
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

fn is_pinned_rustc(version: &str) -> bool {
    let mut tokens = version.split_whitespace();
    tokens.next() == Some("rustc") && tokens.next() == Some(REQUIRED_RUST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_pin_matches_the_exact_stable_patch_not_a_prefix() {
        assert!(is_pinned_rustc("rustc 1.98.1 (release hash)"));
        for version in [
            "rustc 1.98.0 (known vtable regression)",
            "rustc 1.98.10 (different patch)",
            "rustc 1.98.1-nightly (different channel)",
            "rustc 1.99.0 (different release)",
            "cargo 1.98.1 (wrong executable)",
            "rustc",
        ] {
            assert!(!is_pinned_rustc(version), "{version}");
        }
    }
}
