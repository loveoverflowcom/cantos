//! Optional, registered bounded proofs. Rust owns scope/model; the Python primitive owns
//! bounded execution and evidence validation. No installer proxy is ever invoked.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use sha2::{Digest, Sha256};

use crate::{
    command::CommandSpec,
    pipeline::Step,
    report::{Report, Status},
    workspace::Workspace,
};

pub const SCRIPT: &str = "scripts/verify_kani.py";
pub const KANI_VERSION: &str = "0.68.0";
pub const CBMC_VERSION: &str = "6.11.0";
pub const RUSTC_VERSION: &str = "rustc 1.100.0-nightly (8925ea358 2026-08-20)";
pub const TOOLCHAIN: &str = "nightly-2026-08-21-x86_64-unknown-linux-gnu";
pub const TARGET: &str = "x86_64-unknown-linux-gnu";
const MAX_CONFIGURATION_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Kernel {
    CatalogRevision,
    ImageBounds,
    WavPayload,
    SubmissionWindow,
    PublicationCommit,
    ListenerAccess,
    ListenerReconcile,
}

impl Kernel {
    pub const ALL: [Self; 7] = [
        Self::CatalogRevision,
        Self::ImageBounds,
        Self::WavPayload,
        Self::SubmissionWindow,
        Self::PublicationCommit,
        Self::ListenerAccess,
        Self::ListenerReconcile,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::CatalogRevision => "catalog-revision",
            Self::ImageBounds => "image-bounds",
            Self::WavPayload => "wav-payload",
            Self::SubmissionWindow => "submission-window",
            Self::PublicationCommit => "publication-commit",
            Self::ListenerAccess => "listener-access",
            Self::ListenerReconcile => "listener-reconcile",
        }
    }

    pub fn harness(self) -> &'static str {
        match self {
            Self::CatalogRevision => "contexts::catalog::domain::revision::kani_proofs::catalog_revision_admission_and_successor",
            Self::ImageBounds => "contexts::media::domain::image::kani_proofs::image_bounds_admission",
            Self::WavPayload => "contexts::media::domain::wav_inspection::chunks::kani_proofs::wav_payload_range_is_checked",
            Self::SubmissionWindow => "contexts::media::domain::lifecycle::kani_proofs::submission_window_admission_and_membership",
            Self::PublicationCommit => "contexts::media::domain::publication::kani_proofs::publication_pointer_and_rights_fail_closed",
            Self::ListenerAccess => "contexts::media::domain::listening::kani_proofs::listener_keyset_and_access_window",
            Self::ListenerReconcile => "contexts::listening::domain::reconcile::kani_proofs::progress_reconciliation_never_blindly_overwrites_or_wraps",
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::CatalogRevision => "services/cantos-api/src/contexts/catalog/domain/revision.rs",
            Self::ImageBounds => "services/cantos-api/src/contexts/media/domain/image.rs",
            Self::WavPayload => {
                "services/cantos-api/src/contexts/media/domain/wav_inspection/chunks.rs"
            }
            Self::SubmissionWindow => "services/cantos-api/src/contexts/media/domain/lifecycle.rs",
            Self::PublicationCommit => {
                "services/cantos-api/src/contexts/media/domain/publication.rs"
            }
            Self::ListenerAccess => "services/cantos-api/src/contexts/media/domain/listening.rs",
            Self::ListenerReconcile => {
                "services/cantos-api/src/contexts/listening/domain/reconcile.rs"
            }
        }
    }

    pub fn covers(self) -> u8 {
        match self {
            Self::CatalogRevision => 6,
            Self::ImageBounds => 13,
            Self::WavPayload => 7,
            Self::SubmissionWindow => 8,
            Self::PublicationCommit => 6,
            Self::ListenerAccess => 5,
            Self::ListenerReconcile => 7,
        }
    }

    fn unreachable(self) -> Vec<String> {
        match self {
            Self::CatalogRevision => vec![
                format!("{}.assertion.1", self.harness()),
                format!("{}.assertion.5", self.harness()),
                "std::alloc::Layout::max_size_for_alignment.assertion.1".into(),
                "std::fmt::Arguments::<'_>::from_str.assertion.1".into(),
            ],
            Self::SubmissionWindow => vec![format!("{}.assertion.3", self.harness())],
            Self::ImageBounds => Vec::new(),
            Self::WavPayload => Vec::new(),
            Self::PublicationCommit => vec![format!("{}.assertion.1", self.harness())],
            Self::ListenerAccess => Vec::new(),
            Self::ListenerReconcile => Vec::new(),
        }
    }

    pub fn command(self, workspace: &Workspace) -> CommandSpec {
        let mut command = primitive(workspace).args([
            "--harness",
            self.harness(),
            "--source",
            self.source(),
            "--unwind",
            "2",
            "--cover-count",
            &self.covers().to_string(),
        ]);
        for id in self.unreachable() {
            command = command.args(["--allow-unreachable".into(), id]);
        }
        command
    }
}

fn compatibility() -> [&'static str; 10] {
    [
        "--kani-version",
        KANI_VERSION,
        "--cbmc-version",
        CBMC_VERSION,
        "--rustc-version",
        RUSTC_VERSION,
        "--toolchain",
        TOOLCHAIN,
        "--target",
        TARGET,
    ]
}

fn primitive(workspace: &Workspace) -> CommandSpec {
    CommandSpec::new("python3", workspace.root())
        .args([SCRIPT])
        .args(compatibility())
}

pub fn probe_command(root: &std::path::Path) -> CommandSpec {
    CommandSpec::new("python3", root)
        .args([SCRIPT, "--probe"])
        .args(compatibility())
}

/// Broadening is deliberate only after explicit opt-in. Default gate selection is unchanged.
pub fn affected(changed: Option<&[String]>, uncertain: bool) -> Vec<Kernel> {
    let Some(paths) = changed else {
        return Kernel::ALL.to_vec();
    };
    if uncertain || paths.is_empty() {
        return Kernel::ALL.to_vec();
    }
    let mut selected = BTreeSet::new();
    for path in paths {
        if matches!(
            path.as_str(),
            "scripts/verify_kani.py"
                | "scripts/test_verify_kani.py"
                | "services/cantos-api/Cargo.toml"
                | "services/cantos-api/build.rs"
                | "packages/contracts/api/build.rs"
                | "services/cantos-api/src/lib.rs"
                | "services/cantos-api/src/contexts/mod.rs"
                | "packages/contracts/api/Cargo.toml"
        ) || path.starts_with("packages/contracts/api/src/")
            || path.starts_with("packages/contracts/schema/")
        {
            selected.extend(Kernel::ALL);
        }
        if path == "services/cantos-api/src/contexts/listening/mod.rs"
            || path.starts_with("services/cantos-api/src/contexts/listening/domain/")
        {
            selected.insert(Kernel::ListenerReconcile);
        }
        for kernel in Kernel::ALL {
            if path == kernel.source() {
                selected.insert(kernel);
            }
        }
        if matches!(
            path.as_str(),
            "services/cantos-api/src/contexts/catalog/mod.rs"
                | "services/cantos-api/src/contexts/catalog/domain/mod.rs"
                | "services/cantos-api/src/contexts/catalog/domain/error.rs"
        ) {
            selected.insert(Kernel::CatalogRevision);
        }
        if matches!(
            path.as_str(),
            "services/cantos-api/src/contexts/media/mod.rs"
                | "services/cantos-api/src/contexts/media/domain/mod.rs"
        ) {
            selected.extend([
                Kernel::ImageBounds,
                Kernel::WavPayload,
                Kernel::SubmissionWindow,
                Kernel::PublicationCommit,
                Kernel::ListenerAccess,
            ]);
        }
        if path == "services/cantos-api/src/contexts/media/domain/wav_inspection/mod.rs" {
            selected.insert(Kernel::WavPayload);
        }
    }
    selected.into_iter().collect()
}

fn configuration_candidates(root: &Path, cargo_home: Option<&Path>) -> BTreeSet<PathBuf> {
    let mut inputs = BTreeSet::from([
        root.join("Cargo.toml"),
        root.join("services/cantos-api/Cargo.toml"),
    ]);
    for parent in root.ancestors() {
        for name in ["config", "config.toml"] {
            inputs.insert(parent.join(".cargo").join(name));
        }
    }
    if let Some(home) = cargo_home {
        for name in ["config", "config.toml"] {
            inputs.insert(home.join(name));
        }
    }
    inputs
}

fn reject_configuration(text: &str, manifest: bool) -> Result<()> {
    let value: toml::Value =
        toml::from_str(text).map_err(|_| anyhow::anyhow!("invalid TOML configuration"))?;
    if manifest {
        for keys in [
            &["workspace", "metadata", "kani"][..],
            &["package", "metadata", "kani"][..],
            &["kani"][..],
        ] {
            let found = keys.iter().try_fold(&value, |value, key| value.get(*key));
            if found.is_some_and(|value| value.as_table().is_none_or(|table| !table.is_empty())) {
                bail!("implicit Kani configuration is not permitted in this fixed model");
            }
        }
    } else {
        if let Some(build) = value.get("build").and_then(toml::Value::as_table) {
            for key in [
                "rustflags",
                "rustc",
                "rustc-wrapper",
                "rustc-workspace-wrapper",
                "target",
            ] {
                if build.contains_key(key) {
                    bail!("Cargo build.{key} overrides the fixed proof model");
                }
            }
        }
        if value
            .get("target")
            .and_then(toml::Value::as_table)
            .is_some_and(|targets| {
                targets
                    .values()
                    .any(|target| target.get("rustflags").is_some())
            })
        {
            bail!("Cargo target rustflags override the fixed proof model");
        }
        if let Some(env) = value.get("env").and_then(toml::Value::as_table) {
            for key in env.keys() {
                if weak_environment_key(key) {
                    bail!("Cargo env.{key} overrides the fixed proof model");
                }
            }
        }
    }
    Ok(())
}

fn weak_environment_key(key: &str) -> bool {
    matches!(
        key,
        "RUSTFLAGS"
            | "CARGO_ENCODED_RUSTFLAGS"
            | "RUSTC"
            | "RUSTC_WRAPPER"
            | "RUSTC_WORKSPACE_WRAPPER"
            | "CARGO_BUILD_TARGET"
            | "CARGO_BUILD_RUSTFLAGS"
            | "CARGO_BUILD_RUSTC"
            | "CARGO_BUILD_RUSTC_WRAPPER"
            | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
    ) || (key.starts_with("CARGO_TARGET_") && key.ends_with("_RUSTFLAGS"))
}

fn read_configuration(reader: impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_CONFIGURATION_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("could not read proof configuration")?;
    if bytes.len() as u64 > MAX_CONFIGURATION_BYTES {
        bail!("proof configuration is too large");
    }
    Ok(bytes)
}

fn approve_configuration(path: &Path) -> Result<Option<String>> {
    let manifest = path.file_name().is_some_and(|name| name == "Cargo.toml");
    let reader = match std::fs::File::open(path) {
        Ok(reader) => reader,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !manifest => {
            return Ok(None);
        }
        Err(error) => return Err(error).context("could not read proof configuration"),
    };
    let bytes = read_configuration(reader)?;
    let text = std::str::from_utf8(&bytes).context("proof configuration is not UTF-8")?;
    reject_configuration(text, manifest)
        .with_context(|| format!("proof configuration {}", path.display()))?;
    // Hash exactly the bytes approved above, never a second independent file read.
    Ok(Some(format!("{:x}", Sha256::digest(&bytes))))
}

fn configuration_home(
    root: &Path,
    cargo_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Result<PathBuf> {
    // Cargo ignores an empty CARGO_HOME. Refuse an unresolved platform-home fallback
    // rather than approving a candidate set that omits the actual global configuration.
    let directory = match cargo_home.filter(|home| !home.is_empty()) {
        Some(home) => PathBuf::from(home),
        None => PathBuf::from(
            home.filter(|home| !home.is_empty())
                .context("fixed proof configuration requires nonempty CARGO_HOME or HOME")?,
        )
        .join(".cargo"),
    };
    std::path::absolute(root.join(directory)).context("could not resolve proof Cargo home")
}

/// Bind approved bytes and the entire candidate-file set before prerequisite probing.
pub fn prepare_configuration(workspace: &Workspace, steps: &mut [Step]) -> Result<()> {
    let root = workspace.root().canonicalize()?;
    let cargo_home = configuration_home(
        &root,
        std::env::var_os("CARGO_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )?;
    let mut approved = BTreeMap::new();
    for path in configuration_candidates(&root, Some(&cargo_home)) {
        let name = path
            .to_str()
            .context("proof configuration path is not UTF-8")?;
        approved.insert(name.to_owned(), approve_configuration(&path)?);
    }
    let snapshot = serde_json::to_string(&approved)?;
    for step in steps {
        if !step.skipped && step.name.starts_with("kani (") {
            for command in &mut step.commands {
                command
                    .args
                    .extend(["--approved-config".into(), snapshot.clone()]);
            }
        }
    }
    Ok(())
}

pub fn configuration_refusal(
    title: &str,
    steps: &[Step],
    error: anyhow::Error,
) -> std::process::ExitCode {
    let mut report = Report::new(title);
    let mut failed = false;
    for step in steps {
        if step.skipped {
            report.record(&step.name, Status::NotRun, "skipped by --skip");
        } else if !failed && step.name.starts_with("kani (") {
            report.record(&step.name, Status::Fail, format!("{error:#}"));
            failed = true;
        } else {
            report.record(
                &step.name,
                Status::NotRun,
                "not started: fixed proof configuration rejected",
            );
        }
    }
    report.print();
    report.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cargo_home_binds_the_actual_default_global_configuration() {
        let root = Path::new("/example/checkout");
        assert_eq!(
            configuration_home(
                root,
                Some(std::ffi::OsStr::new("")),
                Some(std::ffi::OsStr::new("/example/home")),
            )
            .unwrap(),
            PathBuf::from("/example/home/.cargo"),
        );
    }

    #[test]
    fn explicit_cargo_home_preserves_absolute_and_relative_precedence() {
        let root = Path::new("/example/checkout");
        for (cargo_home, expected) in [
            ("/separate/cargo", "/separate/cargo"),
            ("relative-cargo", "/example/checkout/relative-cargo"),
        ] {
            for home in [
                None,
                Some(std::ffi::OsStr::new("")),
                Some(std::ffi::OsStr::new("/ignored")),
            ] {
                assert_eq!(
                    configuration_home(root, Some(std::ffi::OsStr::new(cargo_home)), home).unwrap(),
                    PathBuf::from(expected),
                );
            }
        }
    }

    #[test]
    fn unresolved_platform_home_cannot_omit_global_configuration() {
        for cargo_home in [None, Some(std::ffi::OsStr::new(""))] {
            for home in [None, Some(std::ffi::OsStr::new(""))] {
                assert!(
                    configuration_home(Path::new("/example/checkout"), cargo_home, home).is_err()
                );
            }
        }
    }

    #[test]
    fn configuration_read_consumes_at_most_limit_plus_one_bytes() {
        struct EndlessReader {
            consumed: usize,
        }
        impl Read for EndlessReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                buffer.fill(b' ');
                self.consumed += buffer.len();
                Ok(buffer.len())
            }
        }
        let mut reader = EndlessReader { consumed: 0 };
        let error = read_configuration(&mut reader).unwrap_err();
        assert!(error.to_string().contains("too large"));
        assert_eq!(reader.consumed as u64, MAX_CONFIGURATION_BYTES + 1);
        let exact_limit = vec![b' '; MAX_CONFIGURATION_BYTES as usize];
        assert_eq!(
            read_configuration(exact_limit.as_slice()).unwrap(),
            exact_limit
        );
    }

    #[test]
    fn candidate_set_includes_absent_ancestor_and_cargo_home_configs() {
        let root = Path::new("/example/checkout");
        let home = Path::new("/separate/cargo-home");
        let candidates = configuration_candidates(root, Some(home));
        for parent in root.ancestors() {
            for name in ["config", "config.toml"] {
                assert!(candidates.contains(&parent.join(".cargo").join(name)));
            }
        }
        for name in ["config", "config.toml"] {
            assert!(candidates.contains(&home.join(name)));
        }
        assert!(candidates.contains(&root.join("Cargo.toml")));
        assert!(candidates.contains(&root.join("services/cantos-api/Cargo.toml")));
    }

    #[test]
    fn approval_hashes_exact_validated_bytes_and_records_absence() {
        let directory = std::env::temp_dir().join(format!(
            "cantos-kani-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("config.toml");
        assert_eq!(approve_configuration(&path).unwrap(), None);
        let bytes = b"# approved exact bytes\n[build]\njobs=2\n";
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            approve_configuration(&path).unwrap(),
            Some(format!("{:x}", Sha256::digest(bytes)))
        );
        std::fs::write(&path, "[build]\nrustflags=['--cfg=weakened']\n").unwrap();
        assert!(approve_configuration(&path).is_err());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(approve_configuration(&path).unwrap(), None);
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn prepared_snapshot_is_shared_only_by_active_proof_commands() {
        let workspace = Workspace::discover().unwrap();
        let mut steps = crate::gate::steps(
            &[
                crate::gate::Gate::Repository,
                crate::gate::Gate::Kani(Kernel::CatalogRevision),
                crate::gate::Gate::Kani(Kernel::WavPayload),
                crate::gate::Gate::Kani(Kernel::SubmissionWindow),
            ],
            &workspace,
        );
        steps[3].skipped = true;
        prepare_configuration(&workspace, &mut steps).unwrap();
        let snapshot = |step: &Step| {
            step.commands[0]
                .args
                .windows(2)
                .find(|args| args[0] == "--approved-config")
                .map(|args| args[1].clone())
        };
        assert!(snapshot(&steps[0]).is_none());
        assert!(snapshot(&steps[3]).is_none());
        assert_eq!(snapshot(&steps[1]), snapshot(&steps[2]));
        let approved: BTreeMap<String, Option<String>> =
            serde_json::from_str(&snapshot(&steps[1]).unwrap()).unwrap();
        let manifest = workspace.path("Cargo.toml");
        assert_eq!(
            approved[manifest.to_str().unwrap()],
            Some(format!(
                "{:x}",
                Sha256::digest(std::fs::read(manifest).unwrap())
            ))
        );
        assert!(approved.contains_key("/.cargo/config"));
        assert!(approved.contains_key("/.cargo/config.toml"));
    }

    #[test]
    fn registrations_reach_actual_production_harnesses() {
        let workspace = Workspace::discover().unwrap();
        for kernel in Kernel::ALL {
            let source = std::fs::read_to_string(workspace.path(kernel.source())).unwrap();
            assert!(source.contains("#[cfg(kani)]"));
            assert!(source.contains(&format!(
                "fn {}()",
                kernel.harness().rsplit("::").next().unwrap()
            )));
            assert_eq!(
                source.matches("kani::cover!").count(),
                usize::from(kernel.covers())
            );
            let command = kernel.command(&workspace);
            assert!(command.args.contains(&"--unwind".into()));
            assert!(command.args.contains(&kernel.harness().into()));
            assert_eq!(command.program, "python3");
            assert!(!command
                .args
                .iter()
                .any(|arg| arg.contains("no-default-checks")));
        }
    }

    #[test]
    fn changed_kernels_select_only_registered_affected_propositions() {
        let pick = |paths: &[&str], uncertain| {
            affected(
                Some(&paths.iter().map(|path| (*path).into()).collect::<Vec<_>>()),
                uncertain,
            )
        };
        for kernel in Kernel::ALL {
            assert_eq!(pick(&[kernel.source()], false), [kernel]);
        }
        assert_eq!(
            pick(
                &["services/cantos-api/src/contexts/catalog/domain/error.rs"],
                false
            ),
            [Kernel::CatalogRevision]
        );
        assert_eq!(
            pick(
                &["services/cantos-api/src/contexts/media/domain/mod.rs"],
                false
            ),
            [
                Kernel::ImageBounds,
                Kernel::WavPayload,
                Kernel::SubmissionWindow,
                Kernel::PublicationCommit,
                Kernel::ListenerAccess
            ]
        );
        for path in [
            SCRIPT,
            "scripts/test_verify_kani.py",
            "services/cantos-api/Cargo.toml",
            "services/cantos-api/build.rs",
            "packages/contracts/api/build.rs",
            "packages/contracts/api/src/lib.rs",
        ] {
            assert_eq!(pick(&[path], false), Kernel::ALL);
        }
        assert_eq!(
            pick(
                &[
                    "README.md",
                    "apps/cantos-theatre/web/src/live/request.rs",
                    "services/manifest.toml"
                ],
                false
            ),
            []
        );
        assert_eq!(pick(&["Cargo.lock"], true), Kernel::ALL);
        assert_eq!(pick(&[], false), Kernel::ALL);
        assert_eq!(affected(None, false), Kernel::ALL);
    }

    #[test]
    fn implicit_configuration_cannot_weaken_the_fixed_model() {
        for text in [
            "[workspace.metadata.kani.flags]\nno-unwinding-checks=true",
            "[package.metadata.kani]\nflags={default-checks=false}",
            "kani={flags={solver='other'}}",
        ] {
            assert!(reject_configuration(text, true).is_err());
        }
        assert!(reject_configuration("[workspace]\nresolver='3'", true).is_ok());
        assert!(reject_configuration("[workspace.metadata.kani]", true).is_ok());
        for text in [
            "[build]\nrustflags=['--cfg=other']",
            "[target.x86_64-unknown-linux-gnu]\nrustflags=[]",
            "[env]\nRUSTC_WRAPPER='private-value'",
            "[build]\ntarget='other'",
        ] {
            assert!(reject_configuration(text, false).is_err());
        }
        assert!(reject_configuration("[alias]\nxtask='run --package xtask --'", false).is_ok());
        assert!(!format!(
            "{:#}",
            reject_configuration("[env]\nRUSTFLAGS='secret-value'", false).unwrap_err()
        )
        .contains("secret-value"));
    }
}
