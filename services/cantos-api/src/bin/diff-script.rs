//! Compare two Script IR versions by stable ID and print a versioned JSON report.
//! Reads only: it never edits an input, calls a provider or records a revision.
use std::{
    env,
    ffi::OsString,
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use cantos_api::script_ir::{
    diff_scripts, read_script, ReadError, ScriptContent, MAX_DOCUMENT_BYTES,
};

const USAGE: &str = "usage: diff-script <before.json> <after.json>";
/// Longest list of validator diagnostics printed for one rejected input.
const MAX_DIAGNOSTICS_SHOWN: usize = 20;

enum Command {
    Help,
    Diff { before: PathBuf, after: PathBuf },
}

/// Exit code 1: the inputs are readable but cannot be compared.
/// Exit code 2: usage, an unreadable or oversized file, or a failed write.
enum Failure {
    Rejected(String),
    Unusable(String),
}

fn main() -> ExitCode {
    let (before, after) = match parse_args(env::args_os().skip(1)) {
        Ok(Command::Diff { before, after }) => (before, after),
        Ok(Command::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&before, &after) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Rejected(message)) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
        Err(Failure::Unusable(message)) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn run(before_path: &Path, after_path: &Path) -> Result<(), Failure> {
    let before = load("before", before_path)?;
    let after = load("after", after_path)?;
    let report = diff_scripts(&before, &after)
        .map_err(|mismatch| Failure::Rejected(format!("scripts cannot be compared: {mismatch}")))?;
    let mut json = serde_json::to_vec_pretty(&report)
        .map_err(|error| Failure::Unusable(format!("report could not be encoded: {error}")))?;
    json.push(b'\n');
    io::stdout()
        .lock()
        .write_all(&json)
        .map_err(|error| Failure::Unusable(format!("report could not be written: {error}")))
}

fn load(label: &str, path: &Path) -> Result<ScriptContent, Failure> {
    let bytes = read_bounded(path)
        .map_err(|message| Failure::Unusable(format!("{label} {}: {message}", path.display())))?;
    read_script(&bytes).map_err(|error| {
        Failure::Rejected(format!(
            "{label} {}: Script IR rejected: {}",
            path.display(),
            describe(&error)
        ))
    })
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Command, String> {
    let mut paths = Vec::new();
    for arg in args {
        match arg.to_str() {
            Some("-h" | "--help") => return Ok(Command::Help),
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unexpected option: {flag}"));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    match <[PathBuf; 2]>::try_from(paths) {
        Ok([before, after]) => Ok(Command::Diff { before, after }),
        Err(paths) => Err(format!(
            "expected exactly two input files, found {}",
            paths.len()
        )),
    }
}

/// Refuses an oversized file before reading it, and again if it grows while it is read.
/// The limit is the Script IR document bound, so a larger file could not be admitted anyway.
fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let unreadable = |error: io::Error| format!("input could not be read: {error}");
    let file = File::open(path).map_err(unreadable)?;
    let metadata = file.metadata().map_err(unreadable)?;
    if !metadata.is_file() {
        return Err("input is not a regular file".into());
    }
    let too_large = || format!("input exceeds the {MAX_DOCUMENT_BYTES}-byte Script IR limit");
    if metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

fn describe(error: &ReadError) -> String {
    match error {
        ReadError::DocumentTooLarge { max_bytes } => {
            format!("document exceeds {max_bytes} bytes")
        }
        ReadError::InvalidDocument {
            line,
            column,
            detail,
        } => format!("invalid JSON at line {line}, column {column}: {detail}"),
        ReadError::MissingSchemaVersion => "missing schema_version".into(),
        ReadError::UnsupportedSchemaVersion { found, supported } => {
            format!("unsupported schema_version {found}; this reader supports {supported}")
        }
        ReadError::Shape(issues) => list("shape", issues),
        ReadError::Semantic(issues) => list("semantic", issues),
        ReadError::NonCanonicalDocument => "document is not canonical".into(),
    }
}

/// One JSON object per diagnostic, so the path and code can be read or piped onward.
fn list(kind: &str, issues: &[impl serde::Serialize]) -> String {
    let mut text = format!("{} {kind} issue(s)", issues.len());
    for issue in issues.iter().take(MAX_DIAGNOSTICS_SHOWN) {
        let line = serde_json::to_string(issue).unwrap_or_else(|_| "<unprintable>".into());
        text.push_str("\n  ");
        text.push_str(&line);
    }
    if issues.len() > MAX_DIAGNOSTICS_SHOWN {
        text.push_str(&format!(
            "\n  ... and {} more",
            issues.len() - MAX_DIAGNOSTICS_SHOWN
        ));
    }
    text
}
