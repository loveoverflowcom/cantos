//! The verification gate catalog. `verify`, `preflight` and `ci` all select from these gates,
//! so one definition owns each check's commands and prerequisites. Scripts under `scripts/`
//! remain the implementation primitives; gates only invoke them.

use std::collections::BTreeSet;

use clap::ValueEnum;

use crate::{
    build::{self, Profile},
    cli::VerifySuite,
    command::CommandSpec,
    config::{App, API_PACKAGE, WASM_TARGET},
    pipeline::Step,
    tools::Tool,
    workspace::Workspace,
};

/// Gate identities for `--skip`; declaration order is execution order (cheap checks first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum)]
pub enum GateKind {
    Repository,
    RustFormat,
    ScriptIrReference,
    RustLint,
    RustTests,
    ContractTests,
    WavReference,
    WebLint,
    WebBuild,
    Mutation,
    Postgres,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    Workspace,
    Packages(BTreeSet<String>),
}

impl Scope {
    fn cargo_args(&self) -> Vec<String> {
        match self {
            Scope::Workspace => vec!["--workspace".into()],
            Scope::Packages(packages) => packages
                .iter()
                .flat_map(|package| ["-p".to_owned(), package.clone()])
                .collect(),
        }
    }

    fn label(&self) -> String {
        match self {
            Scope::Workspace => "workspace".into(),
            Scope::Packages(packages) => packages.iter().cloned().collect::<Vec<_>>().join(", "),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Gate {
    Repository,
    RustFormat,
    ScriptIrReference,
    RustLint(Scope),
    RustTests(Scope),
    ContractTests,
    WavReference,
    WebLint(BTreeSet<App>),
    WebBuild(App),
    Mutation,
    Postgres { keep: bool },
}

/// Integration test targets of `cantos-api` that pin wire, Script IR, WAV and TTS receipt
/// contracts. The PostgreSQL tests are excluded; they belong to [`Gate::Postgres`].
pub const API_CONTRACT_TESTS: [&str; 11] = [
    "script_ir",
    "studio_contract",
    "import_contract",
    "production_contract",
    "script_diff",
    "cli_file_admission",
    "wav_inspection",
    "tts_receipt",
    "tts_attempt",
    "tts_audio",
    "tts_neural_receipt",
];

impl Gate {
    pub fn kind(&self) -> GateKind {
        match self {
            Gate::Repository => GateKind::Repository,
            Gate::RustFormat => GateKind::RustFormat,
            Gate::ScriptIrReference => GateKind::ScriptIrReference,
            Gate::RustLint(_) => GateKind::RustLint,
            Gate::RustTests(_) => GateKind::RustTests,
            Gate::ContractTests => GateKind::ContractTests,
            Gate::WavReference => GateKind::WavReference,
            Gate::WebLint(_) => GateKind::WebLint,
            Gate::WebBuild(_) => GateKind::WebBuild,
            Gate::Mutation => GateKind::Mutation,
            Gate::Postgres { .. } => GateKind::Postgres,
        }
    }

    pub fn name(&self) -> String {
        let kind = self
            .kind()
            .to_possible_value()
            .map(|value| value.get_name().to_owned())
            .unwrap_or_default();
        match self {
            Gate::RustLint(scope) | Gate::RustTests(scope) => {
                format!("{kind} ({})", scope.label())
            }
            Gate::WebLint(apps) => {
                let names: Vec<_> = apps.iter().map(|app| app.cli_name()).collect();
                format!("{kind} ({})", names.join(", "))
            }
            Gate::WebBuild(app) => format!("{kind} ({})", app.cli_name()),
            _ => kind,
        }
    }

    pub fn tools(&self) -> Vec<Tool> {
        match self {
            Gate::Repository => vec![Tool::Python3, Tool::Git],
            Gate::RustFormat => vec![Tool::Cargo, Tool::Rustfmt],
            Gate::ScriptIrReference => vec![Tool::Python3],
            Gate::RustLint(_) => vec![Tool::Cargo, Tool::Clippy],
            Gate::RustTests(_) | Gate::ContractTests => vec![Tool::Cargo],
            Gate::WavReference | Gate::Mutation => vec![Tool::Cargo, Tool::Python3],
            Gate::WebLint(_) => vec![Tool::Cargo, Tool::Clippy, Tool::Wasm32Target],
            Gate::WebBuild(_) => vec![Tool::Cargo, Tool::Wasm32Target, Tool::Trunk],
            Gate::Postgres { .. } => vec![
                Tool::Cargo,
                Tool::Python3,
                Tool::Initdb,
                Tool::PgCtl,
                Tool::Psql,
            ],
        }
    }

    pub fn commands(&self, workspace: &Workspace) -> Vec<CommandSpec> {
        let root = workspace.root();
        let python = |args: &[&str]| CommandSpec::new("python3", root).args(args.iter().copied());
        let cargo = |args: &[&str]| CommandSpec::new("cargo", root).args(args.iter().copied());
        match self {
            Gate::Repository => vec![
                python(&["scripts/check_repository.py"]),
                python(&[
                    "-m",
                    "unittest",
                    "discover",
                    "-s",
                    "scripts",
                    "-p",
                    "test_*.py",
                ]),
                CommandSpec::new("git", root).args(["diff", "--check"]),
                CommandSpec::new("git", root).args(["diff", "--cached", "--check"]),
            ],
            Gate::RustFormat => vec![cargo(&["fmt", "--all", "--", "--check"])],
            Gate::ScriptIrReference => vec![python(&["scripts/reference_script_ir.py"])],
            Gate::RustLint(scope) => vec![cargo(&["clippy"]).args(scope.cargo_args()).args([
                "--all-targets",
                "--locked",
                "--",
                "-D",
                "warnings",
            ])],
            Gate::RustTests(scope) => {
                vec![cargo(&["test"]).args(scope.cargo_args()).args(["--locked"])]
            }
            Gate::ContractTests => vec![
                cargo(&["test", "--locked", "-p", "cantos-contracts"]),
                cargo(&["test", "--locked", "-p", API_PACKAGE])
                    .args(API_CONTRACT_TESTS.iter().flat_map(|test| ["--test", test])),
            ],
            Gate::WavReference => vec![
                cargo(&[
                    "build",
                    "--locked",
                    "-p",
                    API_PACKAGE,
                    "--bin",
                    "inspect-wav",
                ]),
                python(&["scripts/reference_wav_inspection.py"]),
            ],
            Gate::WebLint(apps) => vec![cargo(&["clippy"])
                .args(apps.iter().flat_map(|app| ["-p", app.web_package()]))
                .args(["--target", WASM_TARGET, "--locked", "--", "-D", "warnings"])],
            Gate::WebBuild(app) => vec![build::trunk(workspace, *app, "build", Profile::Dev)],
            Gate::Mutation => vec![python(&["scripts/mutate_script_ir.py"])],
            Gate::Postgres { keep } => {
                vec![python(&["scripts/test_postgres.py"]).arg_if(*keep, "--keep")]
            }
        }
    }

    pub fn step(&self, workspace: &Workspace) -> Step {
        Step::new(self.name(), &self.tools(), self.commands(workspace))
    }
}

/// Every gate at its widest scope: the uncertain-impact and full-preflight selection.
pub fn full() -> Vec<Gate> {
    vec![
        Gate::Repository,
        Gate::RustFormat,
        Gate::ScriptIrReference,
        Gate::RustLint(Scope::Workspace),
        Gate::RustTests(Scope::Workspace),
        Gate::WavReference,
        Gate::WebLint(App::ALL.into_iter().collect()),
        Gate::WebBuild(App::Studio),
        Gate::WebBuild(App::Theatre),
        Gate::Mutation,
        Gate::Postgres { keep: false },
    ]
}

/// The gates behind one `cargo xtask verify <suite>` command.
pub fn verify_suite(suite: &VerifySuite) -> Vec<Gate> {
    match suite {
        VerifySuite::Repository => vec![Gate::Repository],
        VerifySuite::Rust => vec![
            Gate::RustFormat,
            Gate::RustLint(Scope::Workspace),
            Gate::RustTests(Scope::Workspace),
        ],
        VerifySuite::Web => vec![
            Gate::WebLint(App::ALL.into_iter().collect()),
            Gate::WebBuild(App::Studio),
            Gate::WebBuild(App::Theatre),
        ],
        VerifySuite::Contracts => vec![
            Gate::ScriptIrReference,
            Gate::ContractTests,
            Gate::WavReference,
        ],
        VerifySuite::Mutation => vec![Gate::Mutation],
        VerifySuite::Postgres { keep } => vec![Gate::Postgres { keep: *keep }],
    }
}

pub fn steps(gates: &[Gate], workspace: &Workspace) -> Vec<Step> {
    gates.iter().map(|gate| gate.step(workspace)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> Workspace {
        Workspace::discover().unwrap()
    }

    fn lines(gate: &Gate) -> Vec<String> {
        let workspace = workspace();
        gate.commands(&workspace)
            .iter()
            .map(|command| command.display(workspace.root()))
            .collect()
    }

    #[test]
    fn gates_run_the_documented_repository_commands() {
        assert_eq!(
            lines(&Gate::Repository),
            [
                "python3 scripts/check_repository.py",
                "python3 -m unittest discover -s scripts -p 'test_*.py'",
                "git diff --check",
                "git diff --cached --check",
            ]
        );
        assert_eq!(lines(&Gate::RustFormat), ["cargo fmt --all -- --check"]);
        assert_eq!(
            lines(&Gate::RustLint(Scope::Workspace)),
            ["cargo clippy --workspace --all-targets --locked -- -D warnings"]
        );
        assert_eq!(
            lines(&Gate::WebLint(App::ALL.into_iter().collect())),
            ["cargo clippy -p cantos-studio -p cantos-theatre --target wasm32-unknown-unknown --locked -- -D warnings"]
        );
        assert_eq!(
            lines(&Gate::WebBuild(App::Theatre)),
            ["(in apps/cantos/web) NO_COLOR=true trunk build --locked"]
        );
        assert_eq!(
            lines(&Gate::Postgres { keep: true }),
            ["python3 scripts/test_postgres.py --keep"]
        );
    }

    #[test]
    fn package_scopes_are_explicit_and_sorted() {
        let scope = Scope::Packages(["xtask".into(), "cantos-api".into()].into());
        assert_eq!(
            lines(&Gate::RustTests(scope.clone())),
            ["cargo test -p cantos-api -p xtask --locked"]
        );
        assert_eq!(
            Gate::RustLint(scope).name(),
            "rust-lint (cantos-api, xtask)"
        );
    }

    #[test]
    fn every_referenced_script_and_test_target_exists() {
        let workspace = workspace();
        for gate in full()
            .into_iter()
            .chain([Gate::ContractTests, Gate::Postgres { keep: true }])
        {
            for command in gate.commands(&workspace) {
                for arg in command
                    .args
                    .iter()
                    .filter(|arg| arg.starts_with("scripts/"))
                {
                    assert!(workspace.path(arg).is_file(), "{arg}");
                }
            }
        }
        for test in API_CONTRACT_TESTS {
            let path = format!("services/cantos-api/tests/{test}.rs");
            assert!(workspace.path(&path).is_file(), "{path}");
        }
    }

    #[test]
    fn full_selection_is_in_execution_order_and_covers_every_kind_but_focused_tests() {
        let gates = full();
        let mut sorted = gates.clone();
        sorted.sort();
        assert_eq!(gates, sorted);
        let kinds: BTreeSet<_> = gates.iter().map(Gate::kind).collect();
        let all: BTreeSet<_> = GateKind::value_variants().iter().copied().collect();
        // Workspace tests already run every focused contract test target.
        assert_eq!(
            all.difference(&kinds).copied().collect::<Vec<_>>(),
            [GateKind::ContractTests]
        );
    }

    #[test]
    fn verify_suites_cover_the_requested_contracts() {
        let contracts = verify_suite(&VerifySuite::Contracts);
        assert!(contracts.contains(&Gate::ScriptIrReference));
        assert!(contracts.contains(&Gate::WavReference));
        assert_eq!(
            verify_suite(&VerifySuite::Postgres { keep: false }),
            [Gate::Postgres { keep: false }]
        );
    }
}
