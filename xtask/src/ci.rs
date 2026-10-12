//! `cargo xtask ci`: the repository CI contract. GitHub Actions provisions toolchains and
//! PostgreSQL, then calls `cargo xtask ci --job <job>`; locally, `cargo xtask ci` runs every
//! job in order. CI never accepts `--skip`: a missing tool is UNSUPPORTED and fails the run.

use std::process::ExitCode;

use clap::ValueEnum;

use crate::{
    command::{CommandSpec, Runner},
    config::App,
    gate::{self, Gate, Scope},
    pipeline::{self, Step},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CiJob {
    /// Documents, links, skills, Python tests and the independent Script IR oracle.
    Repository,
    /// Formatting, clippy and tests for the whole Cargo workspace, the API with its experimental
    /// DBSP feature, and the WAV oracle.
    Rust,
    /// WASM clippy and Trunk builds of both Web clients and the UI gallery.
    Web,
    /// PostgreSQL integration tests in a new disposable cluster.
    Postgres,
}

impl CiJob {
    pub const ALL: [CiJob; 4] = [CiJob::Repository, CiJob::Rust, CiJob::Web, CiJob::Postgres];

    pub fn gates(self) -> Vec<Gate> {
        match self {
            CiJob::Repository => vec![Gate::Repository, Gate::ScriptIrReference],
            CiJob::Rust => vec![
                Gate::RustFormat,
                Gate::RustLint(Scope::Workspace),
                Gate::RustTests(Scope::Workspace),
                Gate::Dbsp,
                Gate::WavReference,
            ],
            CiJob::Web => vec![
                Gate::WebLint(App::ALL.into_iter().collect()),
                Gate::WebBuild(App::Studio),
                Gate::WebBuild(App::Theatre),
                Gate::UiPreview,
            ],
            CiJob::Postgres => vec![Gate::Postgres { keep: false }],
        }
    }

    /// The PostgreSQL runner builds and tests with `cargo --offline`, so dependencies are
    /// fetched first; the other jobs let Cargo fetch on demand.
    fn needs_fetch(self) -> bool {
        self == CiJob::Postgres
    }
}

pub fn steps(workspace: &Workspace, jobs: &[CiJob]) -> Vec<Step> {
    let mut steps = Vec::new();
    if jobs.iter().any(|job| job.needs_fetch()) {
        steps.push(Step::new(
            "fetch locked dependencies",
            &[Tool::Cargo],
            vec![CommandSpec::new("cargo", workspace.root()).args(["fetch", "--locked"])],
        ));
    }
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
                GateKind::TheatreCmpDemo,
                GateKind::Mutation
            ],
            "mutation stays opt-in; local CMP gates need a provisioned Gradle CI lane"
        );
    }

    #[test]
    fn only_the_postgres_job_prefetches() {
        let workspace = Workspace::discover().unwrap();
        let names = |jobs: &[CiJob]| -> Vec<String> {
            steps(&workspace, jobs)
                .into_iter()
                .map(|step| step.name)
                .collect()
        };
        assert_eq!(
            names(&[CiJob::Postgres]),
            ["fetch locked dependencies", "postgres"]
        );
        assert_eq!(
            names(&[CiJob::Repository]),
            ["repository", "script-ir-reference"]
        );
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
        assert!(workflow.contains("RUSTUP_TOOLCHAIN: 1.87.0"));
    }
}
