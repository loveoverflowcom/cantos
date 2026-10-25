//! Lexical linkage for the public-cover fetch runs only (`statically-checked`).
//!
//! The catalog owns the capability ID and points to its module-owned manifest, which registers
//! the stable run IDs; Quint owns the modeled expectations. Each fetch run must
//! name exactly one native regression over the real cover seams. This membership check does
//! not inspect assertions, replay traces or establish implementation conformance. Existing
//! projection runs are outside this bounded linkage; no registered Rust observer is claimed.

use std::{collections::BTreeSet, fs, path::Path};

const CAPABILITY: &str = "cantos.media.public-cover-projection";
const FETCH_RUN_PREFIX: &str = "cantos.media.public-cover-projection.run.fetch-";
const MARKER: &str = "/// Quint run: ";
const NATIVE_TESTS: &str = "src/contexts/media/domain/public_cover_tests.rs";

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("readable behavior JSON"))
        .expect("behavior file is JSON")
}

fn registered_fetch_runs() -> BTreeSet<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalog = read_json(&root.join("specs/behavior/catalog.json"));
    let capability = catalog["capabilities"]
        .as_array()
        .expect("capability list")
        .iter()
        .find(|capability| capability["id"] == CAPABILITY)
        .expect("the public-cover capability is registered");
    let manifest = read_json(
        &root.join(
            capability["manifestPath"]
                .as_str()
                .expect("the capability points to its module-owned manifest"),
        ),
    );
    assert_eq!(
        manifest["capabilityId"], CAPABILITY,
        "manifest binds another capability"
    );
    manifest["runs"]
        .as_array()
        .expect("run list")
        .iter()
        .map(|run| run["id"].as_str().expect("run ID"))
        .filter(|id| id.starts_with(FETCH_RUN_PREFIX))
        .map(str::to_owned)
        .collect()
}

fn claimed_runs(source: &str) -> Result<Vec<String>, String> {
    let lines: Vec<&str> = source.lines().map(str::trim).collect();
    let mut claimed = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(id) = line.strip_prefix(MARKER) else {
            continue;
        };
        if lines.get(index + 1) != Some(&"#[test]") {
            return Err(format!("`{id}` is not directly above a #[test]"));
        }
        claimed.push(id.trim().to_owned());
    }
    Ok(claimed)
}

fn drift(registered: &BTreeSet<String>, claimed: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for id in registered {
        let count = claimed.iter().filter(|claimed| *claimed == id).count();
        if count != 1 {
            problems.push(format!("{id}: {count} native tests (exactly one required)"));
        }
    }
    for id in claimed {
        if !registered.contains(id) {
            problems.push(format!(
                "{id}: claimed by a native test, not a registered fetch run"
            ));
        }
    }
    problems
}

#[test]
fn every_cover_fetch_run_has_one_native_regression_and_no_unknown_run_is_claimed() {
    let registered = registered_fetch_runs();
    assert!(registered.len() >= 6, "the catalog lost the six fetch runs");
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(NATIVE_TESTS))
        .expect("readable native cover tests");
    let claimed = claimed_runs(&source).expect("well-formed markers");
    let problems = drift(&registered, &claimed);
    assert!(
        problems.is_empty(),
        "cover-fetch linkage drift:\n{}",
        problems.join("\n")
    );
}

#[test]
fn linkage_detects_missing_duplicate_unknown_and_detached_markers() {
    let registered: BTreeSet<String> = ["a", "b", "c"].map(str::to_owned).into();
    let source = format!(
        "{MARKER}a\n#[test]\nfn one() {{}}\n{MARKER}a\n#[test]\nfn two() {{}}\n{MARKER}z\n#[test]\nfn three() {{}}\n{MARKER}b\n#[test]\nfn four() {{}}\n"
    );
    let claimed = claimed_runs(&source).unwrap();
    assert_eq!(
        drift(&registered, &claimed),
        [
            "a: 2 native tests (exactly one required)",
            "c: 0 native tests (exactly one required)",
            "z: claimed by a native test, not a registered fetch run",
        ]
    );
    assert_eq!(
        claimed_runs(&format!("{MARKER}a\nfn not_a_test() {{}}\n")),
        Err("`a` is not directly above a #[test]".into())
    );
}
