//! Keeps the Quint behavior catalog and the native tests that mirror it from drifting apart.
//!
//! The catalog (`specs/behavior/catalog.json`) owns the stable IDs of the capability
//! `cantos.production.forge-attempt`; its `.qnt` model owns the expected behavior. Each
//! registered named run must have exactly one native test that says which run it mirrors, and a
//! native test may not claim a run the catalog does not register. This is a lexical traceability
//! check (`statically-checked`): it proves nothing about what either side asserts, and a model
//! PASS or this test is not implementation conformance.

use std::{collections::BTreeSet, fs, path::Path};

const CAPABILITY: &str = "cantos.production.forge-attempt";
const MARKER: &str = "/// Quint run: ";
const MIRROR: &str = "src/application/produce_tests.rs";

fn registered_runs() -> BTreeSet<String> {
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/behavior/catalog.json");
    let catalog: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(catalog).expect("readable behavior catalog"))
            .expect("catalog is JSON");
    let capability = catalog["capabilities"]
        .as_array()
        .expect("capability list")
        .iter()
        .find(|capability| capability["id"] == CAPABILITY)
        .expect("the Forge attempt capability is registered");
    capability["runs"]
        .as_array()
        .expect("run list")
        .iter()
        .map(|run| run["id"].as_str().expect("run ID").to_owned())
        .collect()
}

/// The run IDs claimed by `source`, each of which must sit directly above a `#[test]`.
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
                "{id}: claimed by a native test, not in the catalog"
            ));
        }
    }
    problems
}

#[test]
fn every_registered_run_has_exactly_one_native_mirror_and_no_unknown_run_is_claimed() {
    let registered = registered_runs();
    assert!(registered.len() >= 13, "the catalog lost registered runs");
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(MIRROR))
        .expect("readable mirror tests");
    let claimed = claimed_runs(&source).expect("well-formed markers");
    let problems = drift(&registered, &claimed);
    assert!(
        problems.is_empty(),
        "behavior drift:\n{}",
        problems.join("\n")
    );
}

#[test]
fn the_check_detects_a_missing_a_duplicate_and_an_unregistered_run() {
    let registered: BTreeSet<String> = ["a", "b", "c"].map(str::to_owned).into();
    let source = format!("{MARKER}a\n#[test]\nfn one() {{}}\n{MARKER}a\n#[test]\nfn two() {{}}\n{MARKER}z\n#[test]\nfn three() {{}}\n{MARKER}b\n#[test]\nfn four() {{}}\n");
    let claimed = claimed_runs(&source).unwrap();
    assert_eq!(
        drift(&registered, &claimed),
        [
            "a: 2 native tests (exactly one required)",
            "c: 0 native tests (exactly one required)",
            "z: claimed by a native test, not in the catalog",
        ]
    );
    assert!(claimed_runs(&format!("{MARKER}a\nfn not_a_test() {{}}\n")).is_err());
}
