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

pub fn select(changed: Option<&[String]>, graph: &PackageGraph) -> Selection {
    let paths = match changed {
        None => return full("changed-only mode was not requested"),
        Some([]) => return full("no changed paths, so impact is unknown"),
        Some(paths) => paths,
    };
    let mut impact = Impact::default();
    for path in paths {
        classify(path, graph, &mut impact);
        if let Some(reason) = impact.broaden.take() {
            return full(&reason);
        }
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
        // The API's binaries, migrations and HTTP host are all exercised by these two.
        gates.insert(Gate::WavReference);
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

fn classify(path: &str, graph: &PackageGraph, impact: &mut Impact) {
    let is_doc = path.ends_with(".md");
    if SHARED_RUST_INPUTS.contains(&path) {
        impact.broaden = Some(format!("{path} is a shared Cargo input"));
    } else if let Some(dir) = VERIFICATION_TOOLING.iter().find(|dir| within(path, dir)) {
        impact.broaden = Some(format!("{path} changes verification tooling ({dir})"));
    } else if within(path, CONTRACTS) {
        // Fixtures (Markdown manuscripts included) and schemas are read by Rust tests.
        if is_doc && !within(path, "packages/contracts/fixtures") {
            return;
        }
        impact.packages.insert(CONTRACTS_PACKAGE.to_owned());
        if within(path, SCRIPT_IR_FIXTURES) {
            impact.gates.insert(Gate::ScriptIrReference);
        }
    } else if within(path, UI_CMP_DIR) {
        // Kotlin is not a Cargo member. Its gate must not depend on metadata or reverse Rust deps.
        if !is_doc {
            impact.gates.insert(Gate::CmpUiPreview);
        }
    } else if within(path, "packages/ui-system") {
        if !is_doc {
            impact.packages.insert(UI_PACKAGE.to_owned());
            if !within(path, "packages/ui-system/leptos") {
                impact.gates.insert(Gate::CmpUiPreview);
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
    } else if within(path, "services/kanidm") {
        impact.notes.insert(
            "services/kanidm is configuration only; no executable gate exists for it".into(),
        );
    } else if App::ALL.iter().any(|app| within(path, app.mobile_dir())) {
        if is_doc {
            impact
                .notes
                .insert("mobile clients have no Gradle project; only documents changed".into());
        } else {
            impact.broaden = Some(format!(
                "{path} is a mobile input but no mobile gate exists yet"
            ));
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
        "ui_tokens.py" | "ui_icons.py" | "studio_tokens.py" | "ui_preview_catalog.py" => {
            impact.packages.insert(UI_PACKAGE.to_owned());
            impact.gates.insert(Gate::CmpUiPreview);
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
        // Compile-time and test inputs of the neural TTS adapter.
        "vieneu_nano_runtime.py" => {
            impact.packages.insert(API_PACKAGE.to_owned());
        }
        _ if within(script, "fixtures") => {
            impact.packages.insert(API_PACKAGE.to_owned());
        }
        // Tested by the repository gate's Python unit tests (or untested helpers).
        "check_repository.py"
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
            &["apps/cantos/mobile/settings.gradle.kts"],
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
                "rust-lint (cantos-api, cantos-contracts, cantos-studio)",
                "rust-tests (cantos-api, cantos-contracts, cantos-studio)",
                "wav-reference",
                "web-lint (cantos-studio)",
                "web-build (cantos-studio)",
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
        assert!(!pick(&["services/cantos-api/src/http.rs"])
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
                "wav-reference",
                "postgres",
            ]
        );
    }

    #[test]
    fn theatre_changes_select_their_web_client() {
        assert_eq!(
            names(&pick(&["apps/cantos/web/index.html"])),
            [
                "repository",
                "rust-format",
                "rust-lint (cantos-theatre)",
                "rust-tests (cantos-theatre)",
                "web-lint (cantos)",
                "web-build (cantos)",
            ]
        );
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
        assert!(pick(&["scripts/fixtures/neural-vietnamese.json"])
            .affected
            .contains("cantos-api"));
        assert_eq!(
            names(&pick(&[
                "scripts/test_check_repository.py",
                "scripts/test_ui_tokens.py"
            ])),
            ["repository", "rust-format"]
        );
    }

    #[test]
    fn configuration_only_and_reserved_locations_are_noted_not_built() {
        let selection = pick(&[
            "services/kanidm/server.toml.example",
            "apps/cantos-studio/mobile/README.md",
        ]);
        assert_eq!(names(&selection), ["repository", "rust-format"]);
        assert_eq!(selection.notes.len(), 2);
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
                ["repository", "rust-format", "cmp-ui-preview"],
                "{path}"
            );
        }
        assert_eq!(
            names(&pick(&["packages/ui-system/cmp/README.md"])),
            ["repository", "rust-format"]
        );
    }

    #[test]
    fn within_matches_whole_path_components() {
        assert!(within("apps/cantos/web/src/lib.rs", "apps/cantos/web"));
        assert!(within("xtask", "xtask"));
        assert!(!within("apps/cantos-studio/web/x", "apps/cantos"));
        assert!(!within("xtaskish/a", "xtask"));
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
                "web-lint (cantos-studio, cantos)",
                "web-build (cantos-studio)",
                "web-build (cantos)",
                "ui-preview",
            ];
            if !path.starts_with("packages/ui-system/leptos/") {
                expected.push("cmp-ui-preview");
            }
            assert_eq!(names(&selection), expected, "{path}");
        }
    }
}
