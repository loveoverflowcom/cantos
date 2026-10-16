//! Pure changed-path gate selection.
//!
//! Each changed path is classified as documentation, a package input, a primitive script with a
//! known gate, or unknown. Package impact follows reverse Cargo dependencies. Shared Cargo
//! configuration, verification tooling (`xtask/`, workflows), an empty change set and any
//! unclassified path broaden to every gate: when impact is uncertain, verify more.

use std::collections::BTreeSet;

use crate::{
    config::{App, API_PACKAGE, UI_CMP_DIR, UI_PACKAGE},
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
const SCRIPT_IR_SOURCES: &str = "services/cantos-api/src/script_ir";
const SCRIPT_IR_TESTS: &str = "services/cantos-api/tests/script_ir.rs";
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
    let mut gates: BTreeSet<Gate> = [Gate::Repository, Gate::RustFormat].into();
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
    if affected.contains(UI_PACKAGE) {
        gates.insert(Gate::UiPreview);
    }
    Selection {
        gates: gates.into_iter().collect(),
        broadened: None,
        affected,
        notes: impact.notes,
    }
}

fn full(reason: &str) -> Selection {
    Selection {
        gates: gate::full(),
        broadened: Some(reason.to_owned()),
        affected: BTreeSet::new(),
        notes: BTreeSet::new(),
    }
}

fn conformance_input(path: &str) -> bool {
    [
        crate::behavior::CONFORMANCE_SCRIPT,
        crate::behavior::OBSERVER,
        "scripts/test_verify_behavior_conformance.py",
        "services/cantos-api/src/contexts/media/domain/lifecycle.rs",
        "services/cantos-api/src/contexts/media/lifecycle_tests.rs",
        "services/cantos-api/src/contexts/production/domain/brief.rs",
        "packages/contracts/api/src/audio_lifecycle.rs",
        "packages/contracts/api/src/audio_intake.rs",
    ]
    .contains(&path)
}

fn behavior_input(path: &str) -> bool {
    !path.ends_with(".md")
        && (within(path, "specs/behavior")
            || within(path, "tools/behavior")
            || path == crate::behavior::SCRIPT
            || path == crate::behavior::CONFORMANCE_SCRIPT)
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
            impact.gates.insert(Gate::TheatreCmpDemo);
        }
        if within(path, SCRIPT_IR_FIXTURES) {
            impact.gates.insert(Gate::ScriptIrReference);
        }
    } else if within(path, UI_CMP_DIR) {
        // Kotlin is not a Cargo member. Its gate must not depend on metadata or reverse Rust deps.
        if !is_doc {
            impact.gates.insert(Gate::CmpUiPreview);
            impact.gates.insert(Gate::TheatreCmpDemo);
        }
    } else if within(path, "packages/ui-system") {
        if !is_doc {
            impact.packages.insert(UI_PACKAGE.to_owned());
            if !within(path, "packages/ui-system/leptos") {
                impact.gates.insert(Gate::CmpUiPreview);
                impact.gates.insert(Gate::TheatreCmpDemo);
            }
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
            impact.gates.insert(Gate::TheatreCmpDemo);
        }
        if within(path, "apps/cantos-theatre/mobile/androidProductionApp") {
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
            impact.gates.insert(Gate::TheatreCmpDemo);
        }
        "reference_protobuf.py" => {
            impact.gates.insert(Gate::ProtobufReference);
            impact.gates.insert(Gate::TheatreCmpDemo);
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
        | "serve_studio.py" => {}
        _ if script.starts_with("test_") && script.ends_with(".py") => {}
        _ => impact.broaden = Some(format!("scripts/{script} has no known impact")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preflight::graph::testing;

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
            "scripts/test_verify_behavior_conformance.py",
            crate::behavior::OBSERVER,
            "services/cantos-api/src/contexts/media/domain/lifecycle.rs",
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
                assert!(
                    selection.gates.contains(&Gate::Postgres { keep: false }),
                    "{path}"
                );
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
            "services/cantos-api/README.md",
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
                "rust-lint (cantos-api, cantos-contracts, cantos-studio, cantos-theatre)",
                "rust-tests (cantos-api, cantos-contracts, cantos-studio, cantos-theatre)",
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
    fn script_ir_fixtures_and_markdown_manuscripts_are_contract_inputs() {
        let selection = pick(&["packages/contracts/fixtures/script-ir/0.1.0/cases.json"]);
        assert!(selection.gates.contains(&Gate::ScriptIrReference));
        assert!(selection.affected.contains("cantos-api"));
        let manuscript = pick(&["packages/contracts/fixtures/manuscript/chapter-vi.md"]);
        assert!(manuscript.affected.contains("cantos-api"));
        assert!(!manuscript.gates.contains(&Gate::ScriptIrReference));
    }

    #[test]
    fn script_ir_sources_add_the_mutation_gate() {
        for path in [
            "services/cantos-api/src/script_ir/validation.rs",
            "services/cantos-api/tests/script_ir.rs",
            "scripts/mutate_script_ir.py",
        ] {
            assert!(pick(&[path]).gates.contains(&Gate::Mutation), "{path}");
        }
        assert!(!pick(&["services/cantos-api/src/interfaces/http/mod.rs"])
            .gates
            .contains(&Gate::Mutation));
    }

    #[test]
    fn an_api_change_does_not_build_unrelated_web_clients() {
        let selection = pick(&["services/cantos-api/migrations/0006_production_inputs.sql"]);
        assert_eq!(
            names(&selection),
            [
                "repository",
                "rust-format",
                "rust-lint (cantos-api)",
                "rust-tests (cantos-api)",
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
    fn media_sources_and_audio_contract_inputs_require_actual_media_inspection() {
        for path in [
            "services/cantos-api/src/contexts/media/audio_intake.rs",
            "services/cantos-api/tests/audio_intake.rs",
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
            assert!(selection.gates.contains(&Gate::TheatreCmpDemo), "{path}");
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
                    "theatre-cmp-demo"
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
    fn theatre_mobile_inputs_select_mock_tests_without_production_or_studio_mobile() {
        for path in [
            "apps/cantos-theatre/mobile/settings.gradle.kts",
            "apps/cantos-theatre/mobile/gradle.properties",
            "apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/DemoState.kt",
            "apps/cantos-theatre/mobile/shared/src/desktopTest/kotlin/DemoTest.kt",
            "apps/cantos-theatre/mobile/preview-sandbox/build.gradle.kts",
            "apps/cantos-theatre/mobile/preview-sandbox/src/commonMain/kotlin/MockStorageDatabase.kt",
            "apps/cantos-theatre/mobile/preview-sandbox/src/commonTest/kotlin/MockStorageDatabaseTest.kt",
            "apps/cantos-theatre/mobile/preview-sandbox/src/androidMain/kotlin/TheatrePreviews.kt",
            "apps/cantos-theatre/mobile/preview-sandbox/src/desktopMain/kotlin/TheatreWorkbench.kt",
            "apps/cantos-theatre/mobile/androidApp/src/main/AndroidManifest.xml",
            "apps/cantos-theatre/mobile/androidProductionApp/build.gradle.kts",
            "apps/cantos-theatre/mobile/iosApp/CantosDemo.xcodeproj/project.pbxproj",
        ] {
            let selection = pick(&[path]);
            assert_eq!(selection.broadened, None, "{path}");
            assert!(selection.affected.is_empty(), "{path}");
            assert_eq!(
                names(&selection),
                ["repository", "rust-format", "theatre-cmp-demo"],
                "{path}"
            );
        }
        assert_eq!(
            names(&pick(&["apps/cantos-theatre/mobile/README.md"])),
            ["repository", "rust-format"]
        );
        let host =
            pick(&["apps/cantos-theatre/mobile/androidProductionApp/src/main/AndroidManifest.xml"]);
        assert!(host
            .notes
            .iter()
            .any(|note| note.contains("theatre-host android-release")));
        assert!(
            !pick(&["apps/cantos-theatre/mobile/androidApp/build.gradle.kts"])
                .notes
                .iter()
                .any(|note| note.contains("theatre-host"))
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
                expected.push("theatre-cmp-demo");
            }
            assert_eq!(names(&selection), expected, "{path}");
        }
    }
}
