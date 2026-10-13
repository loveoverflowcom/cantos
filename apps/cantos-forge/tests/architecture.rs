//! Architecture fitness checks for the Forge crate's internal dependency direction:
//! `domain` ← `application` ← `adapters`/`interfaces`, with `wire` between the core and the edges.
//!
//! These are lexical checks over source text: they catch an accidental import, not every
//! possible way to reach the outside world.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// Paths that would let the pure core perform I/O, observe time or randomness, print, or
/// depend on a framework or an outer layer.
const FORBIDDEN_IN_DOMAIN: &[&str] = &[
    "std::fs",
    "std::io",
    "std::net",
    "std::process",
    "std::env",
    "std::time",
    "std::thread",
    "SystemTime",
    "Instant::",
    "serde",
    "clap",
    "tokio",
    "getrandom",
    "rand::",
    "println!",
    "eprintln!",
    "print!",
    "dbg!",
    "crate::interfaces",
];

/// Crates the domain may name at the start of a `use` path.
const DOMAIN_USE_ROOTS: &[&str] = &["std", "core", "super", "crate", "self", "sha2", "proptest"];

fn rust_sources(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory).expect("readable source directory") {
        let path = entry.expect("readable directory entry").path();
        if path.is_dir() {
            files.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn code_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line.trim()))
        .filter(|(_, line)| !line.starts_with("//"))
}

fn domain_sources() -> Vec<PathBuf> {
    let domain = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/domain");
    let files = rust_sources(&domain);
    assert!(
        files.len() >= 7,
        "expected the domain modules, found {files:?}"
    );
    files
}

#[test]
fn the_domain_performs_no_io_and_imports_no_framework_or_outer_layer() {
    let mut offences = Vec::new();
    for file in domain_sources() {
        let source = fs::read_to_string(&file).expect("UTF-8 source");
        for (number, line) in code_lines(&source) {
            for forbidden in FORBIDDEN_IN_DOMAIN {
                if line.contains(forbidden) {
                    offences.push(format!("{}:{number}: {forbidden}", file.display()));
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "impure domain code:\n{}",
        offences.join("\n")
    );
}

#[test]
fn the_domain_uses_only_std_and_the_digest_crate() {
    let mut offences = Vec::new();
    for file in domain_sources() {
        let source = fs::read_to_string(&file).expect("UTF-8 source");
        for (number, line) in code_lines(&source) {
            let Some(path) = line
                .strip_prefix("pub use ")
                .or_else(|| line.strip_prefix("pub(crate) use "))
                .or_else(|| line.strip_prefix("use "))
            else {
                continue;
            };
            let root = path
                .split([':', ';', '{'])
                .next()
                .unwrap_or_default()
                .trim();
            if !DOMAIN_USE_ROOTS.contains(&root) {
                offences.push(format!("{}:{number}: use {root}", file.display()));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "unexpected domain dependency:\n{}",
        offences.join("\n")
    );
}

#[test]
fn the_checks_detect_a_forbidden_line() {
    let sample =
        "use std::fs::File;\n// use std::net::TcpStream;\nlet now = std::time::SystemTime::now();";
    let found: Vec<_> = code_lines(sample)
        .filter(|(_, line)| FORBIDDEN_IN_DOMAIN.iter().any(|token| line.contains(token)))
        .map(|(number, _)| number)
        .collect();
    assert_eq!(found, [1, 3]);
}

/// What the application layer may not name: effects, frameworks and the layers around it.
const FORBIDDEN_IN_APPLICATION: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "std::time",
    "std::thread",
    "SystemTime",
    "serde",
    "clap",
    "getrandom",
    "println!",
    "eprintln!",
    "print!",
    "dbg!",
    "crate::adapters",
    "crate::interfaces",
    "crate::wire",
];

/// What the wire shapes may not name: anything that runs, and the layers that use them.
const FORBIDDEN_IN_WIRE: &[&str] = &[
    "std::fs",
    "std::io",
    "std::net",
    "std::process",
    "std::env",
    "std::time",
    "clap",
    "crate::application",
    "crate::adapters",
    "crate::interfaces",
];

fn offences_in(files: &[PathBuf], forbidden: &[&str]) -> Vec<String> {
    let mut offences = Vec::new();
    for file in files {
        let source = fs::read_to_string(file).expect("UTF-8 source");
        // Test modules may use temporary directories and clocks; production code may not.
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();
        for (number, line) in code_lines(production) {
            for token in forbidden {
                if line.contains(token) {
                    offences.push(format!("{}:{number}: {token}", file.display()));
                }
            }
        }
    }
    offences
}

#[test]
fn the_application_layer_performs_no_effects_and_names_no_outer_layer() {
    let application = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/application");
    let files = rust_sources(&application);
    assert!(
        files.len() >= 5,
        "expected the use-case modules, found {files:?}"
    );
    let offences = offences_in(&files, FORBIDDEN_IN_APPLICATION);
    assert!(
        offences.is_empty(),
        "application depends outward:\n{}",
        offences.join("\n")
    );
}

#[test]
fn the_wire_shapes_run_nothing_and_name_no_layer_that_uses_them() {
    let wire = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/wire.rs");
    let offences = offences_in(&[wire], FORBIDDEN_IN_WIRE);
    assert!(
        offences.is_empty(),
        "wire depends outward:\n{}",
        offences.join("\n")
    );
}

#[test]
fn the_application_check_detects_a_forbidden_line() {
    let sample = "use std::fs::File;\nuse crate::adapters::FsRuntime;\n";
    let found = code_lines(sample)
        .filter(|(_, line)| {
            FORBIDDEN_IN_APPLICATION
                .iter()
                .any(|token| line.contains(token))
        })
        .count();
    assert_eq!(found, 2);
}
