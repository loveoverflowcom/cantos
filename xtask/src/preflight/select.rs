//! Pure changed-path gate selection.
//!
//! Each changed path is classified as documentation, a package input, a primitive script with a
//! known gate, or unknown. Package impact follows reverse Cargo dependencies. Shared Cargo
//! configuration, verification tooling (`xtask/`, workflows), an empty change set and any
//! unclassified path broaden to every gate: when impact is uncertain, verify more.
//!
//! The local DEV/CHECKPOINT profile ([`select_local`]) reuses the same classification and graph
//! but plans only static repository checks, formatting, affected Rust lint/tests and WASM lint.
//! Everything else the handoff selection would run on the same diff is listed as deferred.

use std::collections::BTreeSet;

use crate::{
    config::{
        App, API_PACKAGE, FORGE_PACKAGE, IDENTITY_PACKAGE, TURSO_STORE_PACKAGE, UI_CMP_DIR,
        UI_PACKAGE,
    },
    gate::{self, Gate, Scope},
};

use super::graph::PackageGraph;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub gates: Vec<Gate>,
    /// Why every gate was selected, when the selection was broadened.
    pub broadened: Option<String>,
    pub affected: BTreeSet<String>,
    pub notes: BTreeSet<String>,
    /// Local profile only: handoff gates for the same diff that this profile does not run.
    pub deferred: Vec<Gate>,
}

#[derive(Default)]
struct Impact {
    packages: BTreeSet<String>,
    apps: BTreeSet<App>,
    gates: BTreeSet<Gate>,
    notes: BTreeSet<String>,
    broaden: Option<String>,
}

const SHARED_RUST_INPUTS: [&str; 8] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
    "rustfmt.toml",
    ".rustfmt.toml",
    "clippy.toml",
    ".clippy.toml",
];
const VERIFICATION_TOOLING: [&str; 3] = [".cargo", "xtask", ".github/workflows"];
const DOCUMENT_TREES: [&str; 5] = ["docs", "templates", ".agents", ".claude", ".github"];
const REPOSITORY_FILES: [&str; 3] = [".editorconfig", ".gitignore", ".env.example"];
const CONTRACTS: &str = "packages/contracts";
const CONTRACTS_PACKAGE: &str = "cantos-contracts";
const SCRIPT_IR_SOURCES: &str = "services/cantos-server/src/script_ir";
const SCRIPT_IR_TESTS: &str = "services/cantos-server/tests/script_ir.rs";
const SCRIPT_IR_FIXTURES: &str = "packages/contracts/fixtures/script-ir";

pub fn within(path: &str, dir: &str) -> bool {
    path == dir
        || path
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub fn select_with_kani(
    changed: Option<&[String]>,
    graph: &PackageGraph,
    enabled: bool,
) -> Selection {
    let mut selection = select(changed, graph);
    if enabled {
        let kernels = crate::kani::affected(changed, selection.broadened.is_some());
        if kernels.is_empty() {
            selection.notes.insert(
                "Kani not selected: no registered kernel affected; no proof was run".into(),
            );
        }
        selection.gates.extend(kernels.into_iter().map(Gate::Kani));
        selection.gates.sort();
    }
    selection
}

pub fn select(changed: Option<&[String]>, graph: &PackageGraph) -> Selection {
    let paths = match changed {
        None => return full("changed-only mode was not requested"),
        Some([]) => return full("no changed paths, so impact is unknown"),
        Some(paths) => paths,
    };
    let conformance_changed = paths.iter().any(|path| conformance_input(path));
    let behavior_changed = conformance_changed || paths.iter().any(|path| behavior_input(path));
    let mut impact = Impact::default();
    for path in paths {
        classify(path, graph, &mut impact);
        if let Some(reason) = impact.broaden.take() {
            let mut selection = full(&reason);
            if behavior_changed {
                selection.gates.push(Gate::Behavior {
                    capability: None,
                    setup: false,
                    conformance: conformance_changed,
                });
                selection.gates.sort();
            }
            return selection;
        }
    }

    if conformance_changed {
        impact
            .gates
            .retain(|gate| !matches!(gate, Gate::Behavior { .. }));
        impact.gates.insert(Gate::Behavior {
            capability: None,
            setup: false,
            conformance: true,
        });
    }
    let affected = graph.dependents_closure(&impact.packages);
    let mut gates: BTreeSet<Gate> = [Gate::Repository { base: None }, Gate::RustFormat].into();
    gates.extend(impact.gates);
    if !affected.is_empty() {
        let scope = Scope::Packages(affected.clone());
        gates.insert(Gate::RustLint(scope.clone()));
        gates.insert(Gate::RustTests(scope));
    }
    let mut apps = impact.apps;
    apps.extend(
        App::ALL
            .into_iter()
            .filter(|app| affected.contains(app.web_package())),
    );
    if !apps.is_empty() {
        gates.extend(apps.iter().map(|app| Gate::WebBuild(*app)));
        gates.insert(Gate::WebLint(apps));
    }
    if affected.contains(API_PACKAGE) {
        // The API's binaries, migrations and HTTP host are all exercised by these, and its
        // experimental DBSP feature must keep compiling and passing the same tests.
        gates.insert(Gate::Dbsp);
        gates.insert(Gate::WavReference);
        gates.insert(Gate::AudioInspection);
        gates.insert(Gate::Postgres { keep: false });
    }
    if affected.contains(TURSO_STORE_PACKAGE) || affected.contains(IDENTITY_PACKAGE) {
        gates.insert(Gate::Turso);
    }
    if affected.contains(UI_PACKAGE) {
        gates.insert(Gate::UiPreview);
    }
    Selection {
        gates: gates.into_iter().collect(),
        broadened: None,
        affected,
        notes: impact.notes,
        deferred: Vec::new(),
    }
}

fn full(reason: &str) -> Selection {
    Selection {
        gates: gate::full(),
        broadened: Some(reason.to_owned()),
        affected: BTreeSet::new(),
        notes: BTreeSet::new(),
        deferred: Vec::new(),
    }
}

/// The local DEV/CHECKPOINT plan for a collected change set. Package impact keeps normal, dev
/// and build reverse dependencies. Tooling code (`xtask/`, `scripts/`, workflows) adds the
/// repository gate's Python regression suite; documentation under `xtask/` stays a document.
/// Shared Cargo inputs and unclassified paths broaden to every local check, never to the
/// handoff campaigns. An empty change set plans only the always-on checks.
pub fn select_local(paths: &[String], graph: &PackageGraph) -> Selection {
    let handoff = select(Some(paths), graph);
    let mut impact = Impact::default();
    let mut tooling = false;
    let broadened = paths
        .iter()
        .find_map(|path| classify_local(path, graph, &mut impact, &mut tooling));

    let mut gates: BTreeSet<Gate> = [Gate::RustFormat].into();
    let mut affected = BTreeSet::new();
    if broadened.is_some() {
        gates.insert(Gate::Repository { base: None });
        gates.insert(Gate::RustLint(Scope::Workspace));
        gates.insert(Gate::RustTests(Scope::Workspace));
        gates.insert(Gate::WebLint(App::ALL.into_iter().collect()));
    } else {
        gates.insert(if tooling {
            Gate::Repository { base: None }
        } else {
            Gate::RepositoryStatic { base: None }
        });
        affected = graph.dependents_closure(&impact.packages);
        if !affected.is_empty() {
            let scope = Scope::Packages(affected.clone());
            gates.insert(Gate::RustLint(scope.clone()));
            gates.insert(Gate::RustTests(scope));
        }
        let apps: BTreeSet<App> = App::ALL
            .into_iter()
            .filter(|app| affected.contains(app.web_package()))
            .collect();
        if !apps.is_empty() {
            gates.insert(Gate::WebLint(apps));
        }
    }

    let deferred: Vec<Gate> = handoff
        .gates
        .into_iter()
        .filter(|gate| !gates.contains(gate))
        .collect();
    let mut notes = impact.notes;
    if paths.is_empty() {
        notes.insert("no changed paths: only the always-on local checks run".into());
    }
    if gates
        .iter()
        .any(|gate| matches!(gate, Gate::RustLint(_) | Gate::RustTests(_)))
    {
        notes.insert("rust-lint/rust-tests check the listed packages with their default features (plus `cantos-theatre/preview` when that package is selected); no other feature lane is covered".into());
    }
    notes.extend(
        deferred
            .iter()
            .filter_map(deferred_coverage)
            .map(str::to_owned),
    );
    Selection {
        gates: gates.into_iter().collect(),
        broadened,
        affected,
        notes,
        deferred,
    }
}

/// Returns a broadening reason, or records the path's local impact.
fn classify_local(
    path: &str,
    graph: &PackageGraph,
    impact: &mut Impact,
    tooling: &mut bool,
) -> Option<String> {
    if SHARED_RUST_INPUTS.contains(&path) || within(path, ".cargo") {
        return Some(format!("{path} is a shared Cargo input"));
    }
    if within(path, "xtask") {
        // xtask fixtures are test inputs, whatever their extension.
        if !path.ends_with(".md") || within(path, "xtask/fixtures") {
            *tooling = true;
            if let Some(package) = graph.owner(path) {
                impact.packages.insert(package.name.clone());
            }
        }
        return None;
    }
    if within(path, ".github/workflows") {
        // Workflows are read by xtask's CI tests and by the Python repository tests.
        *tooling = true;
        impact.packages.insert("xtask".to_owned());
        return None;
    }
    if within(path, "scripts") {
        *tooling = true;
    }
    classify(path, graph, impact);
    impact.broaden.take()
}

/// What a deferred handoff gate leaves uncovered that the local plan could be mistaken for.
fn deferred_coverage(gate: &Gate) -> Option<&'static str> {
    Some(match gate {
        Gate::Repository { .. } => "the repository Python regression suite is not run (no tooling input changed)",
        Gate::Turso => "Turso feature lanes (cantos-identity `persistence`/`service`, cantos-store-turso `testing`) and the capability report are not checked",
        Gate::Dbsp => "cantos-server's `dbsp-experimental` feature is not checked",
        Gate::WebBuild(_) | Gate::UiPreview => "no Trunk build runs: Web coverage is WASM lint only",
        Gate::CmpUiPreview | Gate::TheatreCmpPreview => {
            "Kotlin/CMP sources are not compiled or tested by this profile"
        }
        _ => return None,
    })
}

fn conformance_input(path: &str) -> bool {
    [
        crate::behavior::CONFORMANCE_SCRIPT,
        crate::behavior::NATIVE_CONFORMANCE_SCRIPT,
        crate::behavior::OBSERVER,
        "scripts/test_verify_behavior_conformance.py",
        "scripts/test_verify_behavior_conformance_native.py",
        "services/cantos-server/src/contexts/media/domain/lifecycle.rs",
        "services/cantos-server/src/contexts/media/lifecycle_tests.rs",
        "services/cantos-server/src/contexts/production/domain/brief.rs",
        "packages/contracts/api/src/audio_lifecycle.rs",
        "packages/contracts/api/src/audio_intake.rs",
        crate::behavior::MAILBOX_OBSERVER,
        crate::behavior::ADMISSION_OBSERVER,
        // Native Identity auth: the production seams the observers call (`validate_fact` included),
        // the checked constructors and wire types they build inputs with, and the observer's parent.
        "services/cantos-identity/src/lib.rs",
        "services/cantos-identity/src/domain/mailbox_proof.rs",
        "services/cantos-identity/src/domain/ids.rs",
        "services/cantos-identity/src/domain/security_version.rs",
        "services/cantos-identity/src/domain/admission.rs",
        "services/cantos-server/src/platform/identity_auth/decision.rs",
        "packages/contracts/identity-rpc/src/lib.rs",
        "packages/contracts/identity-rpc/src/generated.rs",
    ]
    .contains(&path)
}

fn behavior_input(path: &str) -> bool {
    !path.ends_with(".md")
        && (within(path, "specs/behavior")
            || within(path, "tools/behavior")
            || path == crate::behavior::SCRIPT
            || path == crate::behavior::CONFORMANCE_SCRIPT
            || path == crate::behavior::NATIVE_CONFORMANCE_SCRIPT)
}

fn classify(path: &str, graph: &PackageGraph, impact: &mut Impact) {
    let is_doc = path.ends_with(".md");
    if SHARED_RUST_INPUTS.contains(&path) {
        impact.broaden = Some(format!("{path} is a shared Cargo input"));
    } else if let Some(dir) = VERIFICATION_TOOLING.iter().find(|dir| within(path, dir)) {
        impact.broaden = Some(format!("{path} changes verification tooling ({dir})"));
    } else if behavior_input(path) {
        impact.gates.insert(Gate::Behavior {
            capability: None,
            setup: false,
            conformance: false,
        });
    } else if within(path, "packages/contracts/identity-rpc") {
        // The Identity RPC contract is its own Cargo member and shares no file with the
        // Studio/Theatre contracts crate, so it selects only itself and its dependents.
        if !is_doc {
            if let Some(package) = graph.owner(path) {
                impact.packages.insert(package.name.clone());
            }
        }
    } else if within(path, CONTRACTS) {
        // Fixtures (Markdown manuscripts included) and schemas are read by Rust tests.
        if is_doc && !within(path, "packages/contracts/fixtures") {
            return;
        }
        impact.packages.insert(CONTRACTS_PACKAGE.to_owned());
        if within(path, "packages/contracts/protobuf")
            || within(path, "packages/contracts/fixtures/protobuf")
            || within(path, "packages/contracts/fixtures/artwork")
            || within(path, "packages/contracts/api/src/protobuf")
            || path == "packages/contracts/api/tests/protobuf_wire.rs"
            || path == "packages/contracts/api/tests/artwork_wire.rs"
        {
            impact.gates.insert(Gate::ProtobufReference);
            impact.gates.insert(Gate::TheatreCmpPreview);
        }
        if within(path, SCRIPT_IR_FIXTURES) {
            impact.gates.insert(Gate::ScriptIrReference);
        }
    } else if within(path, UI_CMP_DIR) {
        // Kotlin is not a Cargo member. Its gate must not depend on metadata or reverse Rust deps.
        if !is_doc {
            impact.gates.insert(Gate::CmpUiPreview);
            impact.gates.insert(Gate::TheatreCmpPreview);
        }
    } else if within(path, "packages/ui-system") {
        if !is_doc {
            impact.packages.insert(UI_PACKAGE.to_owned());
            if !within(path, "packages/ui-system/leptos") {
                impact.gates.insert(Gate::CmpUiPreview);
                impact.gates.insert(Gate::TheatreCmpPreview);
            }
        }
    } else if within(path, "services/cantos-forge-functions") {
        if !is_doc {
            impact.packages.insert(FORGE_PACKAGE.to_owned());
            impact.gates.insert(Gate::ForgeFunctions);
        }
    } else if let Some(package) = graph.owner(path) {
        if path != format!("{}/README.md", package.dir) {
            impact.packages.insert(package.name.clone());
        }
        if within(path, SCRIPT_IR_SOURCES) || path == SCRIPT_IR_TESTS {
            impact.gates.insert(Gate::Mutation);
        }
    } else if let Some(script) = path.strip_prefix("scripts/") {
        classify_script(script, impact);
    } else if within(path, "infra/firebase/hosting") {
        // The tracked Hosting policy is parsed by xtask's own tests (policy validation and the
        // deploy coordinator), so its tests are the impact. No gate reaches Firebase.
        if !is_doc {
            impact.packages.insert("xtask".to_owned());
            impact.notes.insert("the Hosting policy is validated by xtask's tests; `deploy firebase` is never run by a gate".into());
        }
    } else if within(path, "infra/edge") {
        impact.notes.insert("edge configuration is statically checked by repository; no Podman, DNS or Caddy process is started".into());
    } else if within(path, "services/kanidm")
        || within(path, "services/postgresql")
        || path == "services/manifest.toml"
    {
        impact.notes.insert("service inventory/examples are statically validated by repository; no runtime is started".into());
    } else if within(path, crate::config::THEATRE_MOBILE_DIR) {
        if !is_doc {
            impact.gates.insert(Gate::TheatreCmpPreview);
        }
        if within(path, "apps/cantos-theatre/mobile/androidApp") {
            impact.notes.insert("production Android release bytes are inspected only by explicit `cargo xtask theatre-host android-release`; preflight runs the JVM mock gate".into());
        }
    } else if is_doc
        || REPOSITORY_FILES.contains(&path)
        || DOCUMENT_TREES.iter().any(|dir| within(path, dir))
    {
        // Covered by the repository gate, which always runs.
    } else {
        impact.broaden = Some(format!("{path} has no known impact"));
    }
}

fn classify_script(script: &str, impact: &mut Impact) {
    match script {
        "ui_tokens.py"
        | "ui_icons.py"
        | "ui_logos.py"
        | "studio_tokens.py"
        | "ui_preview_catalog.py" => {
            impact.packages.insert(UI_PACKAGE.to_owned());
            impact.gates.insert(Gate::CmpUiPreview);
            impact.gates.insert(Gate::TheatreCmpPreview);
        }
        "reference_protobuf.py" => {
            impact.gates.insert(Gate::ProtobufReference);
            impact.gates.insert(Gate::TheatreCmpPreview);
            impact.packages.insert(CONTRACTS_PACKAGE.to_owned());
        }
        _ if within(script, "protobuf-oracle") => {
            impact.gates.insert(Gate::ProtobufReference);
        }
        "reference_script_ir.py" => {
            impact.gates.insert(Gate::ScriptIrReference);
        }
        "mutate_script_ir.py" => {
            impact.gates.insert(Gate::Mutation);
        }
        "reference_wav_inspection.py" => {
            impact.gates.insert(Gate::WavReference);
        }
        "test_postgres.py" | "production_oracle.py" | "cantos_adaptation_tool.py" => {
            impact.gates.insert(Gate::Postgres { keep: false });
        }
        // Tested by the repository gate's Python unit tests (or untested helpers).
        "check_repository.py"
        | "render_compose_preview.py"
        | "prepare_story_workspace.py"
        | "benchmark_revision_reads.py"
        | "verify_native_isolation.py"
        | "serve_studio.py"
        | "seed_theatre.py"
        | "apk_size.py" => {}
        _ if script.starts_with("test_") && script.ends_with(".py") => {}
        _ => impact.broaden = Some(format!("scripts/{script} has no known impact")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gate::GateKind, preflight::graph::testing};

    fn pick(paths: &[&str]) -> Selection {
        let paths: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
        select(Some(&paths), &testing::cantos())
    }

    fn names(selection: &Selection) -> Vec<String> {
        selection.gates.iter().map(Gate::name).collect()
    }

    #[test]
    fn registered_observation_inputs_select_conformance_without_losing_native_impact() {
        for path in [
            crate::behavior::CONFORMANCE_SCRIPT,
            crate::behavior::NATIVE_CONFORMANCE_SCRIPT,
            "scripts/test_verify_behavior_conformance.py",
            "scripts/test_verify_behavior_conformance_native.py",
            crate::behavior::OBSERVER,
            "services/cantos-server/src/contexts/media/domain/lifecycle.rs",
            "services/cantos-server/src/platform/identity_auth/decision.rs",
            crate::behavior::ADMISSION_OBSERVER,
            "packages/contracts/identity-rpc/src/generated.rs",
            "services/cantos-identity/src/lib.rs",
            "services/cantos-identity/src/domain/mailbox_proof.rs",
            "services/cantos-identity/src/domain/ids.rs",
            crate::behavior::MAILBOX_OBSERVER,
        ] {
            let selection = pick(&[path]);
            assert!(
                selection.gates.contains(&Gate::Behavior {
                    capability: None,
                    setup: false,
                    conformance: true
                }),
                "{path}"
            );
            assert_eq!(
                selection
                    .gates
                    .iter()
                    .filter(|gate| matches!(gate, Gate::Behavior { .. }))
                    .count(),
                1
            );
            if path.starts_with("services/") {
                assert!(
                    selection
                        .gates
                        .iter()
                        .any(|gate| matches!(gate, Gate::RustTests(_))),
                    "{path}"
                );
            }
            if path.starts_with("services/cantos-server/") {
                assert!(
                    selection.gates.contains(&Gate::Postgres { keep: false }),
                    "{path}"
                );
            }
            if path.starts_with("services/cantos-identity/") {
                assert!(selection.gates.contains(&Gate::Turso), "{path}");
            }
        }
        let mut expected = gate::full();
        expected.push(Gate::Behavior {
            capability: None,
            setup: false,
            conformance: true,
        });
        expected.sort();
        for paths in [
            [crate::behavior::OBSERVER, "xtask/src/gate.rs"],
            ["xtask/src/gate.rs", crate::behavior::OBSERVER],
        ] {
            assert_eq!(pick(&paths).gates, expected);
        }
    }

    #[test]
    fn registered_behavior_inputs_select_model_only_without_infrastructure() {
        for path in [
            "specs/behavior/catalog.json",
            "specs/behavior/media/private_audio_admission.qnt",
            "tools/behavior/package-lock.json",
            "tools/behavior/.npmrc",
            "scripts/verify_behavior.py",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            assert_eq!(
                names(&selection),
                ["repository", "behavior", "rust-format"],
                "{path}"
            );
        }
        assert_eq!(
            names(&pick(&["specs/behavior/inventory.md"])),
            ["repository", "rust-format"]
        );
    }

    #[test]
    fn mixed_behavior_and_tooling_changes_preserve_behavior_in_both_orders() {
        let mut expected = gate::full();
        expected.push(Gate::Behavior {
            capability: None,
            setup: false,
            conformance: false,
        });
        expected.sort();
        for paths in [
            [
                "xtask/src/gate.rs",
                "specs/behavior/media/private_audio_admission.qnt",
            ],
            [
                "specs/behavior/media/private_audio_admission.qnt",
                "xtask/src/gate.rs",
            ],
        ] {
            let selection = pick(&paths);
            assert!(selection.broadened.is_some(), "{paths:?}");
            assert_eq!(selection.gates, expected, "{paths:?}");
        }
    }

    #[test]
    fn documents_select_only_the_always_on_gates() {
        let selection = pick(&[
            "README.md",
            "docs/architecture/overview.md",
            ".agents/skills/cantos-engineering/SKILL.md",
            "services/cantos-server/README.md",
            "packages/contracts/adaptation-v1.md",
            "packages/ui-system/README.md",
            ".github/pull_request_template.md",
            "docs/evidence/casting-production-read-results.json",
        ]);
        assert_eq!(selection.broadened, None);
        assert_eq!(names(&selection), ["repository", "rust-format"]);
    }

    #[test]
    fn uncertain_impact_broadens_to_every_gate() {
        assert_eq!(select(None, &testing::cantos()).gates, gate::full());
        for paths in [
            &[][..],
            &["Cargo.lock"],
            &["xtask/src/main.rs"],
            &[".github/workflows/repository-checks.yml"],
            &[".cargo/config.toml"],
            &["scripts/new_tool.py"],
            &["Makefile"],
            &["docs/README.md", "rust-toolchain.toml"],
        ] {
            let selection = pick(paths);
            assert_eq!(selection.gates, gate::full(), "{paths:?}");
            assert!(selection.broadened.is_some(), "{paths:?}");
        }
    }

    #[test]
    fn contract_changes_reach_every_dependent_package() {
        let selection = pick(&["packages/contracts/api/src/production.rs"]);
        assert_eq!(
            names(&selection),
            [
                "repository",
                "rust-format",
                "rust-lint (cantos-contracts, cantos-server, cantos-studio, cantos-theatre, cantos-web-auth)",
                "rust-tests (cantos-contracts, cantos-server, cantos-studio, cantos-theatre, cantos-web-auth)",
                "dbsp",
                "wav-reference",
                "audio-inspection",
                "web-lint (cantos-studio, cantos-theatre)",
                "web-build (cantos-studio)",
                "web-build (cantos-theatre)",
                "postgres",
            ]
        );
    }

    #[test]
    fn web_auth_changes_select_both_consumers_without_server_or_native_gates() {
        let selection = pick(&["packages/web-auth/src/core.rs"]);
        assert_eq!(
            names(&selection),
            [
                "repository",
                "rust-format",
                "rust-lint (cantos-studio, cantos-theatre, cantos-web-auth)",
                "rust-tests (cantos-studio, cantos-theatre, cantos-web-auth)",
                "web-lint (cantos-studio, cantos-theatre)",
                "web-build (cantos-studio)",
                "web-build (cantos-theatre)",
            ]
        );
    }

    #[test]
    fn script_ir_fixtures_and_markdown_manuscripts_are_contract_inputs() {
        let selection = pick(&["packages/contracts/fixtures/script-ir/0.1.0/cases.json"]);
        assert!(selection.gates.contains(&Gate::ScriptIrReference));
        assert!(selection.affected.contains("cantos-server"));
        let manuscript = pick(&["packages/contracts/fixtures/manuscript/chapter-vi.md"]);
        assert!(manuscript.affected.contains("cantos-server"));
        assert!(!manuscript.gates.contains(&Gate::ScriptIrReference));
    }

    #[test]
    fn script_ir_sources_add_the_mutation_gate() {
        for path in [
            "services/cantos-server/src/script_ir/validation.rs",
            "services/cantos-server/tests/script_ir.rs",
            "scripts/mutate_script_ir.py",
        ] {
            assert!(pick(&[path]).gates.contains(&Gate::Mutation), "{path}");
        }
        assert!(
            !pick(&["services/cantos-server/src/adapters/inbound/http/mod.rs"])
                .gates
                .contains(&Gate::Mutation)
        );
    }

    #[test]
    fn an_api_change_does_not_build_unrelated_web_clients() {
        let selection = pick(&["services/cantos-server/schema/0001_current.sql"]);
        assert_eq!(
            names(&selection),
            [
                "repository",
                "rust-format",
                "rust-lint (cantos-server)",
                "rust-tests (cantos-server)",
                "dbsp",
                "wav-reference",
                "audio-inspection",
                "postgres",
            ]
        );
    }

    #[test]
    fn theatre_changes_select_their_web_client() {
        assert_eq!(
            names(&pick(&["apps/cantos-theatre/web/index.html"])),
            [
                "repository",
                "rust-format",
                "rust-lint (cantos-theatre)",
                "rust-tests (cantos-theatre)",
                "web-lint (cantos-theatre)",
                "web-build (cantos-theatre)",
            ]
        );
    }

    #[test]
    fn identity_inputs_select_their_rust_package_and_persistence() {
        for path in [
            "services/cantos-identity/src/lib.rs",
            "services/cantos-identity/src/domain/security_version.rs",
            "services/cantos-identity/src/domain/admission.rs",
            "services/cantos-identity/src/persistence/mod.rs",
            "services/cantos-identity/src/persistence/schema.sql",
            "services/cantos-identity/Cargo.toml",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            // The mailbox observer's parent module and the checked constructors it builds
            // inputs with are registered conformance inputs.
            let behavior: &[&str] = if path.ends_with("/src/lib.rs")
                || path.ends_with("/security_version.rs")
                || path.ends_with("/admission.rs")
            {
                &["behavior"]
            } else {
                &[]
            };
            let expected: Vec<&str> = ["repository"]
                .into_iter()
                .chain(behavior.iter().copied())
                .chain([
                    "rust-format",
                    "rust-lint (cantos-identity)",
                    "rust-tests (cantos-identity)",
                    "turso",
                ])
                .collect();
            assert_eq!(names(&selection), expected, "{path}");
        }
    }

    #[test]
    fn identity_rpc_contract_inputs_select_it_and_identity_with_the_turso_gate() {
        for path in [
            "packages/contracts/identity-rpc/src/lib.rs",
            "packages/contracts/identity-rpc/src/generated.rs",
            "packages/contracts/identity-rpc/Cargo.toml",
            "packages/contracts/identity-rpc/tests/wire.rs",
            "packages/contracts/identity-rpc/identity_v1.proto",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            // `validate_fact` in lib.rs is a production seam of the session-admission observer,
            // and the generated wire types are the facts and reads it decodes.
            let behavior: &[&str] = if path.ends_with("identity-rpc/src/lib.rs")
                || path.ends_with("identity-rpc/src/generated.rs")
            {
                &["behavior"]
            } else {
                &[]
            };
            let expected: Vec<&str> = ["repository"]
                .into_iter()
                .chain(behavior.iter().copied())
                .chain([
                    "rust-format",
                    "rust-lint (cantos-identity, cantos-identity-rpc)",
                    "rust-tests (cantos-identity, cantos-identity-rpc)",
                    "turso",
                ])
                .collect();
            assert_eq!(names(&selection), expected, "{path}");
        }
        // Its prose contract is a document: repository gate only.
        let document = pick(&["packages/contracts/identity-rpc-v1.md"]);
        assert_eq!(names(&document), ["repository", "rust-format"]);
    }

    #[test]
    fn turso_store_inputs_select_their_package_and_the_capability_gate_only() {
        for path in [
            "packages/store-turso/src/lib.rs",
            "packages/store-turso/src/registry.rs",
            "packages/store-turso/tests/owned_database.rs",
            "packages/store-turso/Cargo.toml",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            assert_eq!(
                names(&selection),
                [
                    "repository",
                    "rust-format",
                    "rust-lint (cantos-identity, cantos-store-turso)",
                    "rust-tests (cantos-identity, cantos-store-turso)",
                    "turso",
                ],
                "{path}"
            );
        }
        // Its README is a document: no package gate and no capability run.
        let readme = pick(&["packages/store-turso/README.md"]);
        assert_eq!(names(&readme), ["repository", "rust-format"]);
    }

    #[test]
    fn forge_service_inputs_select_rust_and_offline_wrapper_without_api_or_database() {
        for path in [
            "services/cantos-forge-functions/src/domain/attempt.rs",
            "services/cantos-forge-functions/firebase/src/boundary.mjs",
            "services/cantos-forge-functions/firebase/test/boundary.test.mjs",
            "services/cantos-forge-functions/firebase/package.json",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            assert_eq!(
                names(&selection),
                [
                    "repository",
                    "rust-format",
                    "rust-lint (cantos-forge)",
                    "rust-tests (cantos-forge)",
                    "forge-functions",
                ],
                "{path}"
            );
            assert_eq!(selection.affected, [FORGE_PACKAGE.to_owned()].into());
        }
        assert_eq!(
            names(&pick(&[
                "services/cantos-forge-functions/firebase/README.md"
            ])),
            ["repository", "rust-format"]
        );
        assert!(pick(&["apps/cantos-forge/src/main.rs"]).broadened.is_some());
    }

    #[test]
    fn media_sources_and_audio_contract_inputs_require_actual_media_inspection() {
        for path in [
            "services/cantos-server/src/contexts/media/audio_intake.rs",
            "services/cantos-server/tests/audio_intake.rs",
            "packages/contracts/api/src/audio_intake.rs",
            "packages/contracts/schema/audio-intake/v1.schema.json",
            "packages/contracts/fixtures/audio-intake/v1/receipt-response.json",
        ] {
            let selection = pick(&[path]);
            assert!(selection.gates.contains(&Gate::AudioInspection), "{path}");
            assert_eq!(selection.broadened, None, "{path}");
        }
        assert!(!pick(&["packages/contracts/studio-audio-intake-v1.md"])
            .gates
            .contains(&Gate::AudioInspection));
        assert!(!pick(&["apps/cantos-theatre/web/src/lib.rs"])
            .gates
            .contains(&Gate::AudioInspection));
    }

    #[test]
    fn protobuf_inputs_select_drift_goldens_and_both_language_consumers() {
        for path in [
            "packages/contracts/protobuf/studio_v1.proto",
            "packages/contracts/protobuf/artwork_v1.proto",
            "packages/contracts/fixtures/protobuf/v1/goldens.json",
            "packages/contracts/fixtures/protobuf/v1/hostile.json",
            "packages/contracts/fixtures/artwork/v1/goldens.json",
            "packages/contracts/fixtures/artwork/v1/hostile.json",
            "packages/contracts/api/src/protobuf/generated.rs",
            "packages/contracts/api/src/protobuf/generated_artwork.rs",
            "packages/contracts/api/tests/artwork_wire.rs",
            "scripts/reference_protobuf.py",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            assert!(selection.gates.contains(&Gate::ProtobufReference), "{path}");
            assert!(selection.gates.contains(&Gate::TheatreCmpPreview), "{path}");
            assert!(selection.affected.contains(CONTRACTS_PACKAGE), "{path}");
        }
        let oracle = pick(&["scripts/protobuf-oracle/rust/build.rs"]);
        assert_eq!(oracle.broadened, None);
        assert!(oracle.gates.contains(&Gate::ProtobufReference));
    }

    #[test]
    fn primitive_scripts_select_the_gate_that_exercises_them() {
        assert_eq!(
            names(&pick(&["scripts/test_postgres.py"])),
            ["repository", "rust-format", "postgres"]
        );
        assert_eq!(
            names(&pick(&["scripts/reference_wav_inspection.py"])),
            ["repository", "rust-format", "wav-reference"]
        );
        assert_eq!(
            names(&pick(&[
                "scripts/test_check_repository.py",
                "scripts/test_ui_tokens.py"
            ])),
            ["repository", "rust-format"]
        );
        // Its offline unit tests run in repository; the seed test itself is a cantos-server test.
        let seed = pick(&["scripts/seed_theatre.py", "scripts/test_seed_theatre.py"]);
        assert_eq!(seed.broadened, None);
        assert_eq!(names(&seed), ["repository", "rust-format"]);
        let isolation = pick(&[
            "scripts/verify_native_isolation.py",
            "scripts/test_verify_native_isolation.py",
        ]);
        assert_eq!(isolation.broadened, None);
        assert_eq!(names(&isolation), ["repository", "rust-format"]);
    }

    #[test]
    fn the_hosting_policy_selects_the_xtask_tests_that_parse_it_and_never_every_gate() {
        let selection = pick(&["infra/firebase/hosting/firebase.json"]);
        assert_eq!(selection.broadened, None);
        assert!(
            selection.affected.contains("xtask"),
            "{:?}",
            selection.affected
        );
        assert_eq!(
            names(&selection),
            [
                "repository",
                "rust-format",
                "rust-lint (xtask)",
                "rust-tests (xtask)"
            ]
        );
        // The documentation beside it is covered by the repository gate alone.
        assert_eq!(
            names(&pick(&["infra/firebase/hosting/README.md"])),
            ["repository", "rust-format"]
        );
    }

    #[test]
    fn configuration_only_and_reserved_locations_are_noted_not_built() {
        let selection = pick(&[
            "services/kanidm/server.toml.example",
            "apps/cantos-theatre/mobile/README.md",
        ]);
        assert_eq!(names(&selection), ["repository", "rust-format"]);
        assert_eq!(selection.notes.len(), 1);
    }

    #[test]
    fn edge_assets_select_static_checks_without_starting_the_service_or_cloud_products() {
        for path in ["infra/edge/Caddyfile", "infra/edge/duck.sh.example"] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None);
            assert_eq!(names(&selection), ["repository", "rust-format"]);
            assert_eq!(selection.notes.len(), 1);
        }
    }

    #[test]
    fn kotlin_preview_inputs_select_the_cmp_gate_without_a_cargo_package() {
        for path in [
            "packages/ui-system/cmp/settings.gradle.kts",
            "packages/ui-system/cmp/build.gradle.kts",
            "packages/ui-system/cmp/gradle.properties",
            "packages/ui-system/cmp/src/commonMain/kotlin/PreviewCatalog.kt",
            "packages/ui-system/cmp/src/desktopMain/kotlin/Main.kt",
            "packages/ui-system/cmp/src/desktopTest/kotlin/PreviewTest.kt",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            assert!(selection.affected.is_empty(), "{path}");
            assert_eq!(
                names(&selection),
                [
                    "repository",
                    "rust-format",
                    "cmp-ui-preview",
                    "theatre-cmp-preview"
                ],
                "{path}"
            );
        }
        assert_eq!(
            names(&pick(&["packages/ui-system/cmp/README.md"])),
            ["repository", "rust-format"]
        );
    }

    #[test]
    fn theatre_mobile_inputs_select_the_preview_gate_without_production_or_studio_mobile() {
        for path in [
            "apps/cantos-theatre/mobile/settings.gradle.kts",
            "apps/cantos-theatre/mobile/gradle.properties",
            "apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/DemoState.kt",
            "apps/cantos-theatre/mobile/shared/src/desktopTest/kotlin/DemoTest.kt",
            "apps/cantos-theatre/mobile/preview-sandbox/build.gradle.kts",
            "apps/cantos-theatre/mobile/preview-sandbox/src/commonMain/kotlin/PreviewOwners.kt",
            "apps/cantos-theatre/mobile/preview-sandbox/src/desktopTest/kotlin/PreviewShellFlowTest.kt",
            "apps/cantos-theatre/mobile/preview-sandbox/src/desktopMain/kotlin/TheatreWorkbench.kt",
            "apps/cantos-theatre/mobile/androidApp/src/main/AndroidManifest.xml",
            "apps/cantos-theatre/mobile/iosApp/Cantos.xcodeproj/project.pbxproj",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            assert!(selection.affected.is_empty(), "{path}");
            assert_eq!(
                names(&selection),
                ["repository", "rust-format", "theatre-cmp-preview"],
                "{path}"
            );
        }
        assert_eq!(
            names(&pick(&["apps/cantos-theatre/mobile/README.md"])),
            ["repository", "rust-format"]
        );
        let host = pick(&["apps/cantos-theatre/mobile/androidApp/src/main/AndroidManifest.xml"]);
        assert!(host
            .notes
            .iter()
            .any(|note| note.contains("theatre-host android-release")));
        assert!(
            pick(&["apps/cantos-theatre/mobile/androidApp/build.gradle.kts"])
                .notes
                .iter()
                .any(|note| note.contains("theatre-host android-release"))
        );
    }

    fn local(paths: &[&str]) -> Selection {
        let paths: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
        select_local(&paths, &testing::cantos())
    }

    fn deferred(selection: &Selection) -> Vec<String> {
        selection.deferred.iter().map(Gate::name).collect()
    }

    const LOCAL_KINDS: [GateKind; 5] = [
        GateKind::Repository,
        GateKind::RustFormat,
        GateKind::RustLint,
        GateKind::RustTests,
        GateKind::WebLint,
    ];

    #[test]
    fn local_documents_select_static_repository_checks_and_format_only() {
        let selection = local(&[
            "README.md",
            "docs/architecture/overview.md",
            ".agents/skills/cantos-engineering/references/verification-cadence.md",
            "services/cantos-server/README.md",
            "xtask/README.md",
        ]);
        assert_eq!(selection.broadened, None);
        assert_eq!(names(&selection), ["repository-static", "rust-format"]);
        // xtask/README.md broadens the handoff selection, never the local one.
        assert!(deferred(&selection).contains(&"repository".to_owned()));
        assert!(deferred(&selection).contains(&"postgres".to_owned()));
        assert!(selection
            .notes
            .iter()
            .any(|note| note.contains("Python regression suite is not run")));
    }

    #[test]
    fn local_server_changes_keep_package_checks_and_defer_every_campaign() {
        for path in [
            "services/cantos-server/src/adapters/inbound/http/mod.rs",
            "services/cantos-server/src/script_ir/validation.rs",
            "services/cantos-server/src/contexts/media/audio_intake.rs",
        ] {
            let selection = local(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            assert_eq!(
                names(&selection),
                [
                    "repository-static",
                    "rust-format",
                    "rust-lint (cantos-server)",
                    "rust-tests (cantos-server)",
                ],
                "{path}"
            );
            for campaign in ["dbsp", "wav-reference", "audio-inspection", "postgres"] {
                assert!(
                    deferred(&selection).contains(&campaign.to_owned()),
                    "{path}"
                );
            }
            assert!(selection
                .notes
                .iter()
                .any(|note| note.contains("dbsp-experimental")));
        }
        assert!(deferred(&local(&[
            "services/cantos-server/src/script_ir/validation.rs"
        ]))
        .contains(&"mutation".to_owned()));
    }

    #[test]
    fn local_theatre_web_changes_add_wasm_lint_without_trunk_or_cmp() {
        let selection = local(&["apps/cantos-theatre/web/src/lib.rs"]);
        assert_eq!(
            names(&selection),
            [
                "repository-static",
                "rust-format",
                "rust-lint (cantos-theatre)",
                "rust-tests (cantos-theatre)",
                "web-lint (cantos-theatre)",
            ]
        );
        assert_eq!(
            deferred(&selection),
            ["repository", "web-build (cantos-theatre)"]
        );
        assert!(selection
            .notes
            .iter()
            .any(|note| note.contains("WASM lint only")));
    }

    #[test]
    fn local_contract_changes_reach_every_dependent_through_the_graph() {
        let selection = local(&["packages/contracts/api/src/production.rs"]);
        assert_eq!(
            names(&selection),
            [
                "repository-static",
                "rust-format",
                "rust-lint (cantos-contracts, cantos-server, cantos-studio, cantos-theatre, cantos-web-auth)",
                "rust-tests (cantos-contracts, cantos-server, cantos-studio, cantos-theatre, cantos-web-auth)",
                "web-lint (cantos-studio, cantos-theatre)",
            ]
        );
    }

    #[test]
    fn local_identity_and_turso_changes_keep_graph_consumers_and_report_feature_gaps() {
        for (path, packages) in [
            ("services/cantos-identity/src/lib.rs", "cantos-identity"),
            (
                "packages/store-turso/src/lib.rs",
                "cantos-identity, cantos-store-turso",
            ),
            (
                "packages/contracts/identity-rpc/src/lib.rs",
                "cantos-identity, cantos-identity-rpc",
            ),
        ] {
            let selection = local(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            assert_eq!(
                names(&selection),
                [
                    "repository-static".to_owned(),
                    "rust-format".to_owned(),
                    format!("rust-lint ({packages})"),
                    format!("rust-tests ({packages})"),
                ],
                "{path}"
            );
            assert_eq!(deferred(&selection), ["repository", "turso"], "{path}");
            assert!(selection
                .notes
                .iter()
                .any(|note| note.contains("`persistence`/`service`")));
            assert!(selection
                .notes
                .iter()
                .any(|note| note.contains("default features")));
        }
    }

    #[test]
    fn local_selection_keeps_dev_dependency_consumers() {
        // The server's Identity/Turso journeys are dev-dependencies; graph edges carry
        // normal, dev and build dependencies alike, so the server stays a consumer.
        let mut graph = testing::cantos();
        graph
            .packages
            .iter_mut()
            .find(|package| package.name == API_PACKAGE)
            .unwrap()
            .deps
            .extend([IDENTITY_PACKAGE.to_owned(), TURSO_STORE_PACKAGE.to_owned()]);
        let selection = select_local(&["packages/store-turso/src/lib.rs".to_owned()], &graph);
        assert_eq!(
            names(&selection),
            [
                "repository-static",
                "rust-format",
                "rust-lint (cantos-identity, cantos-server, cantos-store-turso)",
                "rust-tests (cantos-identity, cantos-server, cantos-store-turso)",
            ]
        );
        for campaign in ["turso", "dbsp", "postgres"] {
            assert!(deferred(&selection).contains(&campaign.to_owned()));
        }
    }

    #[test]
    fn local_tooling_code_keeps_the_repository_regression_suite() {
        for (path, rust) in [
            ("xtask/src/gate.rs", true),
            ("xtask/fixtures/hosting-bundles/README.md", true),
            (".github/workflows/repository-checks.yml", true),
            ("scripts/check_repository.py", false),
            ("scripts/test_check_repository.py", false),
            ("scripts/verify_behavior.py", false),
        ] {
            let selection = local(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            let mut expected = vec!["repository", "rust-format"];
            if rust {
                expected.extend(["rust-lint (xtask)", "rust-tests (xtask)"]);
            }
            assert_eq!(names(&selection), expected, "{path}");
        }
        assert!(deferred(&local(&["scripts/verify_behavior.py"])).contains(&"behavior".to_owned()));
    }

    #[test]
    fn local_shared_or_unknown_inputs_broaden_within_the_profile_only() {
        for path in [
            "Cargo.lock",
            "Cargo.toml",
            "rust-toolchain.toml",
            ".cargo/config.toml",
            "Makefile",
            "scripts/new_tool.py",
        ] {
            let selection = local(&[path, "apps/cantos-theatre/web/src/lib.rs"]);
            assert!(
                selection
                    .broadened
                    .as_deref()
                    .is_some_and(|reason| reason.contains(path)),
                "{path}: {:?}",
                selection.broadened
            );
            assert_eq!(
                names(&selection),
                [
                    "repository",
                    "rust-format",
                    "rust-lint (workspace)",
                    "rust-tests (workspace)",
                    "web-lint (cantos-studio, cantos-theatre)",
                ],
                "{path}"
            );
            assert!(
                deferred(&selection).contains(&"postgres".to_owned()),
                "{path}"
            );
        }
    }

    #[test]
    fn a_truly_empty_local_diff_runs_no_workspace_rust_checks() {
        let selection = select_local(&[], &testing::cantos());
        assert_eq!(selection.broadened, None);
        assert_eq!(names(&selection), ["repository-static", "rust-format"]);
        assert!(selection.affected.is_empty());
        assert!(selection
            .notes
            .iter()
            .any(|note| note.contains("no changed paths")));
        // Handoff still treats an empty diff as unknown impact.
        assert_eq!(select(Some(&[]), &testing::cantos()).gates, gate::full());
    }

    #[test]
    fn local_cmp_changes_report_their_coverage_gap_instead_of_building() {
        let selection = local(&["packages/ui-system/cmp/src/commonMain/kotlin/PreviewCatalog.kt"]);
        assert_eq!(names(&selection), ["repository-static", "rust-format"]);
        assert_eq!(
            deferred(&selection),
            ["repository", "cmp-ui-preview", "theatre-cmp-preview"]
        );
        assert!(selection
            .notes
            .iter()
            .any(|note| note.contains("Kotlin/CMP")));
    }

    #[test]
    fn local_plans_never_contain_a_campaign_gate() {
        let all = [
            "packages/contracts/protobuf/studio_v1.proto",
            "packages/contracts/fixtures/script-ir/0.1.0/cases.json",
            "specs/behavior/catalog.json",
            crate::behavior::OBSERVER,
            "packages/ui-system/shared/tokens.json",
            "services/cantos-forge-functions/firebase/src/boundary.mjs",
            "apps/cantos-theatre/mobile/settings.gradle.kts",
            "services/cantos-identity/src/persistence/mod.rs",
            "scripts/test_postgres.py",
            "scripts/mutate_script_ir.py",
            "infra/firebase/hosting/firebase.json",
        ];
        let mut paths: Vec<&str> = all.to_vec();
        for path in all {
            paths.push(path);
            let selection = local(&[path]);
            assert!(
                selection
                    .gates
                    .iter()
                    .all(|gate| LOCAL_KINDS.contains(&gate.kind())),
                "{path}: {:?}",
                names(&selection)
            );
            for gate in &selection.deferred {
                assert!(!selection.gates.contains(gate), "{path}");
            }
        }
        let combined = local(&paths);
        assert!(combined
            .gates
            .iter()
            .all(|gate| LOCAL_KINDS.contains(&gate.kind())));
    }

    #[test]
    fn the_handoff_selection_is_unchanged_and_has_no_deferred_gates() {
        for paths in [
            &["README.md"][..],
            &["services/cantos-server/src/lib.rs"],
            &["xtask/src/gate.rs"],
            &[],
        ] {
            let selection = pick(paths);
            assert!(selection.deferred.is_empty(), "{paths:?}");
            assert!(!selection
                .gates
                .iter()
                .any(|gate| matches!(gate, Gate::RepositoryStatic { .. })));
        }
        assert_eq!(
            names(&pick(&["xtask/README.md"])),
            names(&select(None, &testing::cantos()))
        );
    }

    #[test]
    fn within_matches_whole_path_components() {
        assert!(within(
            "apps/cantos-theatre/web/src/lib.rs",
            "apps/cantos-theatre/web"
        ));
        assert!(within("xtask", "xtask"));
        assert!(!within("apps/cantos-studio/web/x", "apps/cantos-theatre"));
        assert!(!within("xtaskish/a", "xtask"));
    }

    #[test]
    fn optional_kani_selection_does_not_change_default_or_unrelated_preflight() {
        let path = crate::kani::Kernel::WavPayload.source().to_owned();
        let paths = [path];
        let graph = testing::cantos();
        assert_eq!(
            select_with_kani(Some(&paths), &graph, false),
            select(Some(&paths), &graph)
        );
        let opted = select_with_kani(Some(&paths), &graph, true);
        assert_eq!(
            opted
                .gates
                .iter()
                .filter(|gate| gate.kind() == crate::gate::GateKind::Kani)
                .collect::<Vec<_>>(),
            [&Gate::Kani(crate::kani::Kernel::WavPayload)]
        );
        let docs = select_with_kani(Some(&["README.md".into()]), &graph, true);
        assert!(!docs
            .gates
            .iter()
            .any(|gate| gate.kind() == crate::gate::GateKind::Kani));
        assert!(docs
            .notes
            .iter()
            .any(|note| note.contains("no registered kernel affected")));
        for paths in [None, Some(vec!["Cargo.lock".into()]), Some(Vec::new())] {
            let opted = select_with_kani(paths.as_deref(), &graph, true);
            assert_eq!(
                opted
                    .gates
                    .iter()
                    .filter(|gate| gate.kind() == crate::gate::GateKind::Kani)
                    .count(),
                crate::kani::Kernel::ALL.len()
            );
        }
    }

    #[test]
    fn shared_ui_inputs_reach_both_web_consumers_and_the_gallery() {
        for path in [
            "packages/ui-system/shared/tokens.json",
            "packages/ui-system/shared/icons/material-outlined/manifest.json",
            "packages/ui-system/leptos/foundation.css.in",
            "packages/ui-system/leptos/src/fields.rs",
            "scripts/ui_tokens.py",
            "scripts/ui_icons.py",
            "scripts/ui_logos.py",
            "scripts/studio_tokens.py",
            "scripts/ui_preview_catalog.py",
            "packages/ui-system/shared/preview-catalog.json",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            let mut expected = vec![
                "repository",
                "rust-format",
                "rust-lint (cantos-studio, cantos-theatre, cantos-ui)",
                "rust-tests (cantos-studio, cantos-theatre, cantos-ui)",
                "web-lint (cantos-studio, cantos-theatre)",
                "web-build (cantos-studio)",
                "web-build (cantos-theatre)",
                "ui-preview",
            ];
            if !path.starts_with("packages/ui-system/leptos/") {
                expected.push("cmp-ui-preview");
                expected.push("theatre-cmp-preview");
            }
            assert_eq!(names(&selection), expected, "{path}");
        }
    }
}
