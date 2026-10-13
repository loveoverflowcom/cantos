//! The verification gate catalog. `verify`, `preflight` and `ci` all select from these gates,
//! so one definition owns each check's commands and prerequisites. Scripts under `scripts/`
//! remain the implementation primitives; gates only invoke them.

use std::collections::BTreeSet;

use clap::ValueEnum;

use crate::{
    build::{self, Profile},
    cli::VerifySuite,
    command::CommandSpec,
    config::{App, API_PACKAGE, DBSP_FEATURE, UI_PACKAGE, WASM_TARGET},
    kani::{self, Kernel},
    pipeline::Step,
    theatre_demo,
    tools::Tool,
    ui_preview,
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
    Dbsp,
    ContractTests,
    WavReference,
    AudioInspection,
    WebLint,
    WebBuild,
    UiPreview,
    CmpUiPreview,
    TheatreCmpDemo,
    Mutation,
    Postgres,
    Kani,
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
    /// The API built with its experimental DBSP incremental core: lint and every non-ignored test,
    /// so the feature's added dependency features cannot silently change existing behavior.
    Dbsp,
    ContractTests,
    WavReference,
    /// Actual owned Opus fixtures through the bounded read-only ffmpeg/ffprobe adapter.
    AudioInspection,
    WebLint(BTreeSet<App>),
    WebBuild(App),
    UiPreview,
    CmpUiPreview,
    TheatreCmpDemo,
    Mutation,
    Postgres {
        keep: bool,
    },
    Kani(Kernel),
}

/// Integration test targets of `cantos-api` for metadata, wire, Script IR and read-only media
/// admission. Tool-backed ignored tests belong to `Gate::AudioInspection`; database tests
/// belong to `Gate::Postgres`.
pub const API_CONTRACT_TESTS: [&str; 12] = [
    "script_ir",
    "studio_contract",
    "wire_contract",
    "import_contract",
    "production_contract",
    "script_diff",
    "cli_file_admission",
    "wav_inspection",
    "audio_intake",
    "audio_intake_contract",
    "publication_contract",
    "listener_library_contract",
];

impl Gate {
    pub fn kind(&self) -> GateKind {
        match self {
            Gate::Repository => GateKind::Repository,
            Gate::RustFormat => GateKind::RustFormat,
            Gate::ScriptIrReference => GateKind::ScriptIrReference,
            Gate::RustLint(_) => GateKind::RustLint,
            Gate::RustTests(_) => GateKind::RustTests,
            Gate::Dbsp => GateKind::Dbsp,
            Gate::ContractTests => GateKind::ContractTests,
            Gate::WavReference => GateKind::WavReference,
            Gate::AudioInspection => GateKind::AudioInspection,
            Gate::WebLint(_) => GateKind::WebLint,
            Gate::WebBuild(_) => GateKind::WebBuild,
            Gate::UiPreview => GateKind::UiPreview,
            Gate::CmpUiPreview => GateKind::CmpUiPreview,
            Gate::TheatreCmpDemo => GateKind::TheatreCmpDemo,
            Gate::Mutation => GateKind::Mutation,
            Gate::Postgres { .. } => GateKind::Postgres,
            Gate::Kani(_) => GateKind::Kani,
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
            Gate::Kani(kernel) => format!("kani ({})", kernel.name()),
            _ => kind,
        }
    }

    pub fn tools(&self) -> Vec<Tool> {
        match self {
            Gate::Repository => vec![Tool::Cargo, Tool::Python3, Tool::Git],
            Gate::RustFormat => vec![Tool::Cargo, Tool::Rustfmt],
            Gate::Kani(_) => vec![Tool::Cargo, Tool::Python3, Tool::Git, Tool::Kani],
            Gate::ScriptIrReference => vec![Tool::Python3],
            Gate::RustLint(_) | Gate::Dbsp => vec![Tool::Cargo, Tool::Clippy],
            Gate::RustTests(_) | Gate::ContractTests => vec![Tool::Cargo],
            Gate::WavReference | Gate::Mutation => vec![Tool::Cargo, Tool::Python3],
            Gate::AudioInspection => vec![Tool::Cargo, Tool::Ffmpeg, Tool::Ffprobe],
            Gate::WebLint(_) => vec![Tool::Cargo, Tool::Clippy, Tool::Wasm32Target],
            Gate::WebBuild(_) | Gate::UiPreview => {
                vec![Tool::Cargo, Tool::Wasm32Target, Tool::Trunk]
            }
            Gate::CmpUiPreview => ui_preview::cmp_tools(),
            Gate::TheatreCmpDemo => theatre_demo::tools(),
            Gate::Postgres { .. } => vec![
                Tool::Cargo,
                Tool::Python3,
                Tool::Initdb,
                Tool::PgCtl,
                Tool::Psql,
                Tool::Ffmpeg,
                Tool::Ffprobe,
            ],
        }
    }

    pub fn commands(&self, workspace: &Workspace) -> Vec<CommandSpec> {
        let root = workspace.root();
        let python = |args: &[&str]| CommandSpec::new("python3", root).args(args.iter().copied());
        let cargo = |args: &[&str]| CommandSpec::new("cargo", root).args(args.iter().copied());
        match self {
            Gate::Repository => vec![
                cargo(&[
                    "run", "--quiet", "--locked", "-p", "xtask", "--", "services", "validate",
                ]),
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
            Gate::Kani(kernel) => vec![kernel.command(workspace)],
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
            Gate::Dbsp => vec![
                cargo(&["clippy", "-p", API_PACKAGE, "--features", DBSP_FEATURE]).args([
                    "--all-targets",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ]),
                cargo(&[
                    "test",
                    "-p",
                    API_PACKAGE,
                    "--features",
                    DBSP_FEATURE,
                    "--locked",
                ]),
            ],
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
            Gate::AudioInspection => vec![cargo(&[
                "test",
                "-p",
                API_PACKAGE,
                "--test",
                "audio_intake",
                "--locked",
                "--",
                "--ignored",
            ])],
            Gate::WebLint(apps) => vec![cargo(&["clippy"])
                .args(["-p", UI_PACKAGE])
                .args(apps.iter().flat_map(|app| ["-p", app.web_package()]))
                .args([
                    "--all-targets",
                    "--target",
                    WASM_TARGET,
                    "--features",
                    "cantos-ui/preview",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ])],
            Gate::WebBuild(app) => vec![build::trunk(workspace, *app, "build", Profile::Dev)],
            Gate::UiPreview => vec![ui_preview::leptos_command(workspace, "build")],
            Gate::CmpUiPreview => vec![ui_preview::cmp_command(workspace, "desktopTest")],
            Gate::TheatreCmpDemo => vec![theatre_demo::desktop_command(workspace, "desktopTest")],
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
        Gate::Dbsp,
        Gate::WavReference,
        Gate::AudioInspection,
        Gate::WebLint(App::ALL.into_iter().collect()),
        Gate::WebBuild(App::Studio),
        Gate::WebBuild(App::Theatre),
        Gate::UiPreview,
        Gate::CmpUiPreview,
        Gate::TheatreCmpDemo,
        Gate::Mutation,
        Gate::Postgres { keep: false },
    ]
}

/// The gates behind one `cargo xtask verify <suite>` command.
pub fn verify_suite(suite: &VerifySuite) -> Vec<Gate> {
    match suite {
        VerifySuite::Kani { harness } => {
            let kernels: BTreeSet<_> = if harness.is_empty() {
                kani::Kernel::ALL.into()
            } else {
                harness.iter().copied().collect()
            };
            kernels.into_iter().map(Gate::Kani).collect()
        }
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
            Gate::UiPreview,
        ],
        VerifySuite::Contracts => vec![
            Gate::ScriptIrReference,
            Gate::ContractTests,
            Gate::WavReference,
            Gate::AudioInspection,
        ],
        VerifySuite::Mutation => vec![Gate::Mutation],
        VerifySuite::Dbsp => vec![Gate::Dbsp],
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
                "cargo run --quiet --locked -p xtask -- services validate",
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
            ["cargo clippy -p cantos-ui -p cantos-studio -p cantos-theatre --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview --locked -- -D warnings"]
        );
        assert_eq!(
            lines(&Gate::WebBuild(App::Theatre)),
            ["(in apps/cantos-theatre/web) NO_COLOR=true trunk build --locked"]
        );
        assert_eq!(
            lines(&Gate::UiPreview),
            ["(in packages/ui-system/leptos) NO_COLOR=true trunk build preview.html --locked --dist ../../../target/ui-foundation-preview"]
        );
        assert_eq!(
            lines(&Gate::Postgres { keep: true }),
            ["python3 scripts/test_postgres.py --keep"]
        );
        assert_eq!(
            lines(&Gate::Dbsp),
            [
                "cargo clippy -p cantos-api --features dbsp-experimental --all-targets --locked -- -D warnings",
                "cargo test -p cantos-api --features dbsp-experimental --locked",
            ]
        );
        assert_eq!(verify_suite(&VerifySuite::Dbsp), [Gate::Dbsp]);
    }

    #[test]
    fn web_verification_is_independent_of_the_cmp_toolchain() {
        assert!(!verify_suite(&VerifySuite::Web).contains(&Gate::CmpUiPreview));
        assert!(full().contains(&Gate::CmpUiPreview));
        assert_eq!(
            Gate::CmpUiPreview.tools(),
            [Tool::Gradle, Tool::Java, Tool::CmpGradleProject]
        );
    }

    #[test]
    fn audio_inspection_runs_the_ignored_real_media_target_with_both_tool_prerequisites() {
        assert_eq!(
            lines(&Gate::AudioInspection),
            ["cargo test -p cantos-api --test audio_intake --locked -- --ignored"]
        );
        assert_eq!(
            Gate::AudioInspection.tools(),
            [Tool::Cargo, Tool::Ffmpeg, Tool::Ffprobe]
        );
        assert_eq!(Gate::AudioInspection.name(), "audio-inspection");
        assert!(full().contains(&Gate::AudioInspection));
        assert!(API_CONTRACT_TESTS.contains(&"audio_intake"));
        assert!(API_CONTRACT_TESTS.contains(&"audio_intake_contract"));
    }

    #[test]
    fn either_missing_audio_tool_blocks_inspection_before_any_test_command_runs() {
        use crate::{
            command::testing::RecordingRunner, pipeline, report::Status, tools::testing::FakeTools,
        };

        let workspace = workspace();
        let step = Gate::AudioInspection.step(&workspace);
        for missing in [Tool::Ffmpeg, Tool::Ffprobe] {
            let mut runner = RecordingRunner::default();
            let report = pipeline::execute(
                "audio inspection",
                std::slice::from_ref(&step),
                &FakeTools::without(&[missing]),
                &mut runner,
            );
            assert!(runner.commands.is_empty());
            assert_eq!(report.outcomes[0].status, Status::Unsupported);
            assert!(report.outcomes[0].detail.contains(missing.label()));
            assert!(!report.succeeded());
        }
    }

    #[test]
    fn postgres_audio_intake_checks_declare_the_same_media_tools_up_front() {
        let tools = Gate::Postgres { keep: false }.tools();
        assert!(tools.contains(&Tool::Ffmpeg));
        assert!(tools.contains(&Tool::Ffprobe));
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
    fn kani_is_registered_targeted_and_absent_from_ordinary_gates() {
        let suite = VerifySuite::Kani {
            harness: Vec::new(),
        };
        assert_eq!(verify_suite(&suite), Kernel::ALL.map(Gate::Kani));
        assert_eq!(
            verify_suite(&VerifySuite::Kani {
                harness: vec![Kernel::WavPayload, Kernel::WavPayload]
            }),
            [Gate::Kani(Kernel::WavPayload)]
        );
        assert!(!full().iter().any(|gate| gate.kind() == GateKind::Kani));
        assert!(!verify_suite(&VerifySuite::Rust)
            .iter()
            .any(|gate| gate.kind() == GateKind::Kani));
        for kernel in Kernel::ALL {
            let gate = Gate::Kani(kernel);
            assert_eq!(
                gate.tools(),
                [Tool::Cargo, Tool::Python3, Tool::Git, Tool::Kani]
            );
            assert_eq!(gate.name(), format!("kani ({})", kernel.name()));
            let command = &gate.commands(&workspace())[0];
            assert_eq!(command.args[0], kani::SCRIPT);
            assert!(command
                .args
                .windows(2)
                .any(|args| args == ["--harness", kernel.harness()]));
            assert!(command
                .args
                .windows(2)
                .any(|args| args == ["--unwind", "2"]));
        }
    }

    #[test]
    fn kani_prerequisite_and_failure_use_unchanged_pipeline_statuses() {
        use crate::{
            command::testing::RecordingRunner, pipeline, report::Status, tools::testing::FakeTools,
        };
        let steps = steps(&Kernel::ALL.map(Gate::Kani), &workspace());
        for missing in [Tool::Kani, Tool::Git] {
            let mut runner = RecordingRunner::default();
            let unavailable =
                pipeline::execute("kani", &steps, &FakeTools::without(&[missing]), &mut runner);
            assert!(runner.commands.is_empty());
            assert!(unavailable
                .outcomes
                .iter()
                .all(|outcome| outcome.status == Status::Unsupported));
        }
        let mut runner = RecordingRunner {
            fail_on: Some(Kernel::CatalogRevision.harness().into()),
            ..Default::default()
        };
        let failure = pipeline::execute("kani", &steps, &FakeTools::all(), &mut runner);
        assert_eq!(
            failure
                .outcomes
                .iter()
                .map(|outcome| outcome.status)
                .collect::<Vec<_>>(),
            [
                Status::Fail,
                Status::NotRun,
                Status::NotRun,
                Status::NotRun,
                Status::NotRun,
                Status::NotRun
            ]
        );
        assert_eq!(runner.commands.len(), 1);
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
    fn full_selection_is_ordered_and_excludes_only_focused_tests_and_optional_kani() {
        let gates = full();
        let mut sorted = gates.clone();
        sorted.sort();
        assert_eq!(gates, sorted);
        let kinds: BTreeSet<_> = gates.iter().map(Gate::kind).collect();
        let all: BTreeSet<_> = GateKind::value_variants().iter().copied().collect();
        // Workspace tests already run every focused contract test target.
        assert_eq!(
            all.difference(&kinds).copied().collect::<Vec<_>>(),
            [GateKind::ContractTests, GateKind::Kani]
        );
    }

    #[test]
    fn verify_suites_cover_the_requested_contracts() {
        let contracts = verify_suite(&VerifySuite::Contracts);
        assert!(contracts.contains(&Gate::ScriptIrReference));
        assert!(contracts.contains(&Gate::WavReference));
        assert!(contracts.contains(&Gate::AudioInspection));
        assert_eq!(
            verify_suite(&VerifySuite::Postgres { keep: false }),
            [Gate::Postgres { keep: false }]
        );
    }
}
