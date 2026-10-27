//! `cargo xtask ci`: the repository CI contract. GitHub Actions provisions toolchains and
//! trusted media tools only (no database or other service), then calls `cargo xtask ci --job <job>`; locally, `cargo xtask ci` runs every
//! job in order. CI never accepts `--skip`: a missing tool is UNSUPPORTED and fails the run.

use std::process::ExitCode;

use clap::ValueEnum;

use crate::{
    command::Runner,
    config::App,
    gate::{self, Gate, Scope},
    pipeline::{self, Step},
    tools::ToolProbe,
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CiJob {
    /// Documents, links, skills, Python tests and the independent Script IR oracle.
    Repository,
    /// Formatting, clippy and tests for the whole Cargo workspace (the Server's file-backed
    /// database tests included), offline Forge wrapper tests, the embedded Turso store
    /// foundation, the WAV oracle and read-only Opus inspection. No database service exists.
    Rust,
    /// WASM clippy and Trunk builds of both Web clients and the UI gallery.
    Web,
}

impl CiJob {
    pub const ALL: [CiJob; 3] = [CiJob::Repository, CiJob::Rust, CiJob::Web];

    pub fn gates(self) -> Vec<Gate> {
        match self {
            CiJob::Repository => vec![Gate::Repository { base: None }, Gate::ScriptIrReference],
            CiJob::Rust => vec![
                Gate::RustFormat,
                Gate::ProtobufReference,
                Gate::RustLint(Scope::Workspace),
                Gate::RustTests(Scope::Workspace),
                Gate::ForgeFunctions,
                Gate::Turso,
                Gate::WavReference,
                Gate::AudioInspection,
            ],
            CiJob::Web => vec![
                Gate::WebLint(App::ALL.into_iter().collect()),
                Gate::WebBuild(App::Studio),
                Gate::WebBuild(App::Theatre),
                Gate::UiPreview,
            ],
        }
    }
}

pub fn steps(workspace: &Workspace, jobs: &[CiJob]) -> Vec<Step> {
    let mut steps = Vec::new();
    for job in jobs {
        steps.extend(gate::steps(&job.gates(), workspace));
    }
    steps
}

pub fn run(
    workspace: &Workspace,
    job: Option<CiJob>,
    dry_run: bool,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> ExitCode {
    let jobs = match job {
        Some(job) => vec![job],
        None => CiJob::ALL.to_vec(),
    };
    let steps = steps(workspace, &jobs);
    println!("Cantos CI contract");
    pipeline::print_plan(&steps, workspace.root());
    if dry_run {
        println!("\nDry run: nothing was executed (every step is NOT_RUN).");
        return ExitCode::SUCCESS;
    }
    let report = pipeline::execute("ci", &steps, tools, runner);
    report.print();
    report.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::GateKind;
    use std::collections::BTreeSet;

    #[test]
    fn ci_jobs_cover_full_preflight_except_mutation_and_local_cmp_gates() {
        let ci: BTreeSet<GateKind> = CiJob::ALL
            .iter()
            .flat_map(|job| job.gates())
            .map(|gate| gate.kind())
            .collect();
        let full: BTreeSet<GateKind> = gate::full().iter().map(Gate::kind).collect();
        assert_eq!(
            full.difference(&ci).copied().collect::<Vec<_>>(),
            [
                GateKind::CmpUiPreview,
                GateKind::TheatreCmpPreview,
                GateKind::Mutation
            ],
            "mutation stays opt-in; local CMP gates need a provisioned Gradle CI lane"
        );
    }

    #[test]
    fn no_job_provisions_or_selects_a_database_and_none_prefetches() {
        let workspace = Workspace::discover().unwrap();
        let names = |jobs: &[CiJob]| -> Vec<String> {
            steps(&workspace, jobs)
                .into_iter()
                .map(|step| step.name)
                .collect()
        };
        assert_eq!(
            names(&[CiJob::Repository]),
            ["repository", "script-ir-reference"]
        );
        let every = names(&CiJob::ALL);
        assert!(every.iter().all(|name| !name.contains("postgres")
            && !name.contains("dbsp")
            && !name.contains("fetch")));
        let jobs: Vec<_> = CiJob::ALL
            .iter()
            .map(|job| job.to_possible_value().unwrap().get_name().to_owned())
            .collect();
        assert_eq!(jobs, ["repository", "rust", "web"]);
    }

    #[test]
    fn the_rust_job_runs_actual_media_inspection_separately_from_ordinary_ignored_tests() {
        assert!(CiJob::Rust.gates().contains(&Gate::AudioInspection));
        assert!(!CiJob::Repository.gates().contains(&Gate::AudioInspection));
        assert!(!CiJob::Web.gates().contains(&Gate::AudioInspection));
    }

    #[test]
    fn rust_ci_runs_offline_forge_wrapper_tests_with_node_22_provisioned() {
        assert!(CiJob::Rust.gates().contains(&Gate::ForgeFunctions));
        assert!(!CiJob::Web.gates().contains(&Gate::ForgeFunctions));
        let workspace = Workspace::discover().unwrap();
        let workflow =
            std::fs::read_to_string(workspace.path(".github/workflows/repository-checks.yml"))
                .unwrap();
        assert!(workflow.contains("node-version: '22'"));
        assert!(workflow.contains("package-manager-cache: false"));
    }

    #[test]
    fn forge_probe_qualification_is_independent_pinned_and_read_only() {
        let workspace = Workspace::discover().unwrap();
        let workflow =
            std::fs::read_to_string(workspace.path(".github/workflows/repository-checks.yml"))
                .unwrap();
        let job = workflow
            .split_once("  forge-probe-linux:\n")
            .unwrap()
            .1
            .split_once("  web:\n")
            .unwrap()
            .0;
        for required in [
            "runs-on: ubuntu-24.04",
            "timeout-minutes: 10",
            "permissions:\n      contents: read",
            "ref: ${{ github.event.pull_request.head.sha || github.sha }}",
            "persist-credentials: false",
            "musl-tools=1.2.4-2",
            "rustup target add x86_64-unknown-linux-musl --toolchain 1.98.1",
            "node-version: '22.23.4'",
            "npm_config_registry: https://registry.npmjs.org",
            "cargo xtask forge-probe package",
            "cargo xtask forge-probe inspect",
            "cargo xtask forge-probe qualify",
            "path: artifacts/forge-probe/ci-linux/receipt.json",
        ] {
            assert!(job.contains(required), "missing {required}");
        }
        for forbidden in [
            "needs:",
            "secrets.",
            "id-token:",
            "packages: write",
            "continue-on-error:",
            "firebase deploy",
            "--publish",
            "oci-build",
            "--skip",
        ] {
            assert!(!job.contains(forbidden), "forbidden {forbidden}");
        }
    }

    #[test]
    fn the_manual_qualification_campaign_is_dispatch_only_and_calls_one_xtask_owner() {
        let workspace = Workspace::discover().unwrap();
        let workflow =
            std::fs::read_to_string(workspace.path(".github/workflows/repository-checks.yml"))
                .unwrap();
        let job = workflow
            .split_once("  native-auth-qualification:\n")
            .expect("the manual campaign job exists")
            .1;
        assert!(job.contains(
            "if: github.event_name == 'workflow_dispatch' && inputs.native_auth_qualification"
        ));
        assert!(job.contains("runs-on: ubuntu-24.04"));
        assert_eq!(
            job.matches("cargo xtask verify native-auth-qualification --kani")
                .count(),
            1
        );
        // Provisioning only: the pinned tools, never a copied verifier or a skipped gate.
        for pin in [
            "node-version: '24.19.0'",
            "npm@11.9.0",
            "kani-verifier --version 0.68.0",
        ] {
            assert!(job.contains(pin), "{pin}");
        }
        for forbidden in [
            "--skip",
            "cargo kani --",
            "verify_native_isolation.py",
            "cargo test",
        ] {
            assert!(!job.contains(forbidden), "{forbidden}");
        }
        // No ordinary CI job runs the campaign.
        let ordinary = workflow
            .split_once("  native-auth-qualification:\n")
            .unwrap()
            .0;
        assert!(!ordinary.contains("native-auth-qualification --"));
    }

    #[test]
    fn the_workflow_calls_each_job_through_xtask() {
        let workspace = Workspace::discover().unwrap();
        let workflow =
            std::fs::read_to_string(workspace.path(".github/workflows/repository-checks.yml"))
                .unwrap();
        for job in CiJob::ALL {
            let name = job.to_possible_value().unwrap().get_name().to_owned();
            assert!(
                workflow.contains(&format!("cargo xtask ci --job {name}")),
                "workflow does not run `cargo xtask ci --job {name}`"
            );
        }
        assert_eq!(
            workflow
                .matches("run: rustup show active-toolchain")
                .count(),
            CiJob::ALL.len() + 2,
            "every job, including the two qualification lanes, must select rust-toolchain.toml"
        );
        assert!(
            !workflow.contains("RUSTUP_TOOLCHAIN:"),
            "a workflow override must not bypass the repository pin"
        );
        assert!(!workflow.contains("rustup toolchain install"));
    }
}
