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

/// The closed set of workspace packages that may own a proof kernel. The Python primitive
/// re-checks the same allowlist; no layer accepts a package name supplied by a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Package {
    Api,
    Forge,
    Identity,
}

impl Package {
    pub const ALL: [Self; 3] = [Self::Api, Self::Forge, Self::Identity];

    pub fn name(self) -> &'static str {
        match self {
            Self::Api => "cantos-server",
            Self::Forge => "cantos-forge",
            Self::Identity => "cantos-identity",
        }
    }

    /// Workspace-relative manifest that must exist and carry no implicit Kani configuration.
    pub fn manifest(self) -> &'static str {
        match self {
            Self::Api => "services/cantos-server/Cargo.toml",
            Self::Forge => "services/cantos-forge-functions/Cargo.toml",
            Self::Identity => "services/cantos-identity/Cargo.toml",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|package| package.name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Kernel {
    CatalogRevision,
    ImageBounds,
    WavPayload,
    SubmissionWindow,
    PublicationCommit,
    ListenerAccess,
    ListenerReconcile,
    ByteRange,
    PlaybackGrant,
    PublicCover,
    IdentitySecurityVersion,
    IdentityCompletion,
    IdentityMailboxProof,
    ForgeAttemptLedger,
}

impl Kernel {
    pub const ALL: [Self; 14] = [
        Self::CatalogRevision,
        Self::ImageBounds,
        Self::WavPayload,
        Self::SubmissionWindow,
        Self::PublicationCommit,
        Self::ListenerAccess,
        Self::ListenerReconcile,
        Self::ByteRange,
        Self::PlaybackGrant,
        Self::PublicCover,
        Self::IdentitySecurityVersion,
        Self::IdentityCompletion,
        Self::IdentityMailboxProof,
        Self::ForgeAttemptLedger,
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
            Self::ByteRange => "byte-range",
            Self::PlaybackGrant => "playback-grant",
            Self::PublicCover => "public-cover",
            Self::IdentitySecurityVersion => "identity-security-version",
            Self::IdentityCompletion => "identity-completion",
            Self::IdentityMailboxProof => "identity-mailbox-proof",
            Self::ForgeAttemptLedger => "forge-attempt-ledger",
        }
    }

    /// The workspace package the harness is compiled in; every kernel names one explicitly.
    pub fn package(self) -> Package {
        match self {
            Self::CatalogRevision
            | Self::ImageBounds
            | Self::WavPayload
            | Self::SubmissionWindow
            | Self::PublicationCommit
            | Self::ListenerAccess
            | Self::ListenerReconcile
            | Self::ByteRange
            | Self::PlaybackGrant
            | Self::PublicCover => Package::Api,
            Self::ForgeAttemptLedger => Package::Forge,
            Self::IdentitySecurityVersion
            | Self::IdentityCompletion
            | Self::IdentityMailboxProof => Package::Identity,
        }
    }

    fn in_package(package: Package) -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(move |kernel| kernel.package() == package)
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
            Self::ByteRange => "contexts::media::domain::object_storage::kani_proofs::byte_range_normalization_stays_inside_the_exact_object",
            Self::PlaybackGrant => "contexts::media::domain::playback_grant::kani_proofs::playback_grant_expiry_and_fetch_fail_closed",
            Self::PublicCover => "contexts::media::domain::public_cover::kani_proofs::public_cover_requires_pinned_bound_image_and_current_disclosure",
            Self::IdentitySecurityVersion => "domain::security_version::kani_proofs::security_version_never_wraps_or_leaves_storage_range",
            Self::IdentityCompletion => "domain::admission::kani_proofs::completion_requires_current_active_bound_security_state",
            Self::IdentityMailboxProof => "domain::mailbox_proof::kani_proofs::mailbox_proof_plans_require_explicit_current_bound_unexpired_confirmation",
            Self::ForgeAttemptLedger => "domain::attempt::kani_proofs::attempt_ledger_traces_never_overspend_or_blindly_retry",
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::CatalogRevision => {
                "services/cantos-server/src/contexts/catalog/domain/revision.rs"
            }
            Self::ImageBounds => "services/cantos-server/src/contexts/media/domain/image.rs",
            Self::WavPayload => {
                "services/cantos-server/src/contexts/media/domain/wav_inspection/chunks.rs"
            }
            Self::SubmissionWindow => {
                "services/cantos-server/src/contexts/media/domain/lifecycle.rs"
            }
            Self::PublicationCommit => {
                "services/cantos-server/src/contexts/media/domain/publication.rs"
            }
            Self::ListenerAccess => "services/cantos-server/src/contexts/media/domain/listening.rs",
            Self::ListenerReconcile => {
                "services/cantos-server/src/contexts/listening/domain/reconcile.rs"
            }
            Self::ByteRange => "services/cantos-server/src/contexts/media/domain/object_storage.rs",
            Self::PlaybackGrant => {
                "services/cantos-server/src/contexts/media/domain/playback_grant.rs"
            }
            Self::PublicCover => "services/cantos-server/src/contexts/media/domain/public_cover.rs",
            Self::IdentitySecurityVersion => {
                "services/cantos-identity/src/domain/security_version.rs"
            }
            Self::IdentityCompletion => "services/cantos-identity/src/domain/admission.rs",
            Self::IdentityMailboxProof => {
                "services/cantos-identity/src/domain/mailbox_proof/kani_proofs.rs"
            }
            Self::ForgeAttemptLedger => "services/cantos-forge-functions/src/domain/attempt.rs",
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
            Self::ByteRange => 6,
            Self::PlaybackGrant => 6,
            Self::PublicCover => 6,
            Self::IdentitySecurityVersion => 6,
            Self::IdentityCompletion => 10,
            Self::IdentityMailboxProof => 16,
            Self::ForgeAttemptLedger => 9,
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
            Self::ByteRange => Vec::new(),
            Self::PlaybackGrant => Vec::new(),
            Self::PublicCover => Vec::new(),
            Self::IdentitySecurityVersion
            | Self::IdentityCompletion
            | Self::IdentityMailboxProof => Vec::new(),
            Self::ForgeAttemptLedger => Vec::new(),
        }
    }

    pub fn command(self, workspace: &Workspace) -> CommandSpec {
        let mut command = primitive(workspace).args([
            "--harness",
            self.harness(),
            "--source",
            self.source(),
            "--package",
            self.package().name(),
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
        // The primitive and its tests are shared by every package.
        if matches!(
            path.as_str(),
            "scripts/verify_kani.py" | "scripts/test_verify_kani.py"
        ) {
            selected.extend(Kernel::ALL);
        }
        // API build inputs reach only API kernels; Forge sources reach only the Forge kernel.
        if matches!(
            path.as_str(),
            "services/cantos-server/Cargo.toml"
                | "services/cantos-server/build.rs"
                | "services/cantos-server/src/lib.rs"
                | "services/cantos-server/src/contexts/mod.rs"
        ) || path.starts_with("services/cantos-server/src/contract/")
            || path.starts_with("services/cantos-server/contracts/")
        {
            selected.extend(Kernel::in_package(Package::Api));
        }
        if path == Package::Forge.manifest()
            || path == "services/cantos-forge-functions/src/lib.rs"
            || path.starts_with("services/cantos-forge-functions/src/domain/")
        {
            selected.extend(Kernel::in_package(Package::Forge));
        }
        if matches!(
            path.as_str(),
            "services/cantos-identity/Cargo.toml"
                | "services/cantos-identity/src/lib.rs"
                | "services/cantos-identity/src/domain/mod.rs"
                | "services/cantos-identity/src/domain/ids.rs"
                | "services/cantos-identity/src/domain/security_version.rs"
        ) {
            // Completion and mailbox proof reach the checked version/identifier constructors.
            selected.extend(Kernel::in_package(Package::Identity));
        }
        if path == "services/cantos-identity/src/domain/admission.rs"
            || path == "services/cantos-identity/src/domain/mailbox_proof.rs"
            || path.starts_with("services/cantos-identity/src/domain/mailbox_proof/")
        {
            // Mailbox recovery uses Purpose from admission; its proof module is split by role.
            selected.insert(Kernel::IdentityMailboxProof);
        }
        if path == "services/cantos-server/src/contexts/listening/mod.rs"
            || path.starts_with("services/cantos-server/src/contexts/listening/domain/")
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
            "services/cantos-server/src/contexts/catalog/mod.rs"
                | "services/cantos-server/src/contexts/catalog/domain/mod.rs"
                | "services/cantos-server/src/contexts/catalog/domain/error.rs"
        ) {
            selected.insert(Kernel::CatalogRevision);
        }
        if matches!(
            path.as_str(),
            "services/cantos-server/src/contexts/media/mod.rs"
                | "services/cantos-server/src/contexts/media/domain/mod.rs"
        ) {
            selected.extend([
                Kernel::ImageBounds,
                Kernel::WavPayload,
                Kernel::SubmissionWindow,
                Kernel::PublicationCommit,
                Kernel::ListenerAccess,
                Kernel::ByteRange,
                Kernel::PlaybackGrant,
                Kernel::PublicCover,
            ]);
        }
        if path == "services/cantos-server/src/contexts/media/domain/wav_inspection/mod.rs" {
            selected.insert(Kernel::WavPayload);
        }
    }
    selected.into_iter().collect()
}

/// The workspace manifest and the owning package's manifest are required; the Python primitive
/// re-derives this exact set from the same package name.
fn configuration_candidates(
    root: &Path,
    cargo_home: Option<&Path>,
    package: Package,
) -> BTreeSet<PathBuf> {
    let mut inputs = BTreeSet::from([root.join("Cargo.toml"), root.join(package.manifest())]);
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

/// The package a registered proof command was built for; anything else is refused.
fn command_package(command: &CommandSpec) -> Result<Package> {
    let mut named = command
        .args
        .windows(2)
        .filter(|args| args[0] == "--package")
        .map(|args| args[1].as_str());
    match (named.next(), named.next()) {
        (Some(name), None) => Package::from_name(name)
            .with_context(|| format!("proof command names an unregistered package {name:?}")),
        _ => bail!("proof command must name exactly one registered package"),
    }
}

fn approved_snapshot(root: &Path, cargo_home: &Path, package: Package) -> Result<String> {
    let mut approved = BTreeMap::new();
    for path in configuration_candidates(root, Some(cargo_home), package) {
        let name = path
            .to_str()
            .context("proof configuration path is not UTF-8")?;
        approved.insert(name.to_owned(), approve_configuration(&path)?);
    }
    Ok(serde_json::to_string(&approved)?)
}

/// Bind approved bytes and the entire candidate-file set, per package, before prerequisite
/// probing. Commands of one package share one snapshot; packages never share a snapshot.
pub fn prepare_configuration(workspace: &Workspace, steps: &mut [Step]) -> Result<()> {
    let root = workspace.root().canonicalize()?;
    let cargo_home = configuration_home(
        &root,
        std::env::var_os("CARGO_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )?;
    let active = |step: &Step| !step.skipped && step.name.starts_with("kani (");
    let packages = steps
        .iter()
        .filter(|step| active(step))
        .flat_map(|step| &step.commands)
        .map(command_package)
        .collect::<Result<BTreeSet<_>>>()?;
    let mut snapshots = BTreeMap::new();
    for package in packages {
        snapshots.insert(package, approved_snapshot(&root, &cargo_home, package)?);
    }
    for step in steps {
        if active(step) {
            for command in &mut step.commands {
                let snapshot = snapshots[&command_package(command)?].clone();
                command.args.extend(["--approved-config".into(), snapshot]);
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
        let candidates = configuration_candidates(root, Some(home), Package::Api);
        for parent in root.ancestors() {
            for name in ["config", "config.toml"] {
                assert!(candidates.contains(&parent.join(".cargo").join(name)));
            }
        }
        for name in ["config", "config.toml"] {
            assert!(candidates.contains(&home.join(name)));
        }
        assert!(candidates.contains(&root.join("Cargo.toml")));
        assert!(candidates.contains(&root.join("services/cantos-server/Cargo.toml")));
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
                crate::gate::Gate::Repository { base: None },
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

    /// Independent of `Kernel::package`: the ten kernels registered before the Forge package.
    const API_KERNELS: [Kernel; 10] = [
        Kernel::CatalogRevision,
        Kernel::ImageBounds,
        Kernel::WavPayload,
        Kernel::SubmissionWindow,
        Kernel::PublicationCommit,
        Kernel::ListenerAccess,
        Kernel::ListenerReconcile,
        Kernel::ByteRange,
        Kernel::PlaybackGrant,
        Kernel::PublicCover,
    ];

    fn pick(paths: &[&str], uncertain: bool) -> Vec<Kernel> {
        affected(
            Some(&paths.iter().map(|path| (*path).into()).collect::<Vec<_>>()),
            uncertain,
        )
    }

    #[test]
    fn changed_kernels_select_only_registered_affected_propositions() {
        for kernel in Kernel::ALL {
            let expected = match kernel {
                Kernel::IdentitySecurityVersion => vec![
                    Kernel::IdentitySecurityVersion,
                    Kernel::IdentityCompletion,
                    Kernel::IdentityMailboxProof,
                ],
                Kernel::IdentityCompletion => {
                    vec![Kernel::IdentityCompletion, Kernel::IdentityMailboxProof]
                }
                _ => vec![kernel],
            };
            assert_eq!(pick(&[kernel.source()], false), expected);
        }
        assert_eq!(
            pick(
                &["services/cantos-server/src/contexts/catalog/domain/error.rs"],
                false
            ),
            [Kernel::CatalogRevision]
        );
        assert_eq!(
            pick(
                &["services/cantos-server/src/contexts/media/domain/mod.rs"],
                false
            ),
            [
                Kernel::ImageBounds,
                Kernel::WavPayload,
                Kernel::SubmissionWindow,
                Kernel::PublicationCommit,
                Kernel::ListenerAccess,
                Kernel::ByteRange,
                Kernel::PlaybackGrant,
                Kernel::PublicCover
            ]
        );
        // The shared primitive and its tests reach every package; API build inputs reach
        // only the API kernels (the Forge kernel has its own selection test below).
        for path in [SCRIPT, "scripts/test_verify_kani.py"] {
            assert_eq!(pick(&[path], false), Kernel::ALL);
        }
        for path in [
            "services/cantos-server/Cargo.toml",
            "services/cantos-server/build.rs",
            "services/cantos-server/src/contract/studio/mod.rs",
            "services/cantos-server/contracts/studio/script-ir/0.1.0/script-ir.schema.json",
            "services/cantos-server/src/lib.rs",
            "services/cantos-server/src/contexts/mod.rs",
        ] {
            assert_eq!(pick(&[path], false), API_KERNELS, "{path}");
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
    fn package_registry_is_closed_and_names_the_real_manifests() {
        let workspace = Workspace::discover().unwrap();
        assert_eq!(
            Package::ALL.map(Package::name),
            ["cantos-server", "cantos-forge", "cantos-identity"]
        );
        for package in Package::ALL {
            assert_eq!(Package::from_name(package.name()), Some(package));
            let manifest = std::fs::read_to_string(workspace.path(package.manifest())).unwrap();
            let manifest: toml::Value = toml::from_str(&manifest).unwrap();
            assert_eq!(manifest["package"]["name"].as_str(), Some(package.name()));
        }
        for name in [
            "",
            "cantos-server ",
            "CANTOS-FORGE",
            "cantos-contracts",
            "../cantos-forge",
            "cantos-server,cantos-forge",
        ] {
            assert_eq!(Package::from_name(name), None, "{name:?}");
        }
        assert_eq!(Package::Api.manifest(), "services/cantos-server/Cargo.toml");
        assert_eq!(
            Package::Forge.manifest(),
            "services/cantos-forge-functions/Cargo.toml"
        );
    }

    #[test]
    fn every_kernel_lives_in_the_package_it_names() {
        for kernel in Kernel::ALL {
            let package_source = Path::new(kernel.package().manifest())
                .parent()
                .unwrap()
                .join("src");
            assert!(
                Path::new(kernel.source()).starts_with(&package_source),
                "{} is not under {}",
                kernel.name(),
                package_source.display()
            );
        }
        for kernel in API_KERNELS {
            assert_eq!(kernel.package(), Package::Api);
        }
        assert_eq!(Kernel::ForgeAttemptLedger.package(), Package::Forge);
        assert_eq!(
            Kernel::in_package(Package::Forge).collect::<Vec<_>>(),
            [Kernel::ForgeAttemptLedger]
        );
        assert_eq!(
            Kernel::in_package(Package::Api).collect::<Vec<_>>(),
            API_KERNELS
        );
    }

    #[test]
    fn identity_proofs_select_their_real_shared_construction_dependencies() {
        let identity = [
            Kernel::IdentitySecurityVersion,
            Kernel::IdentityCompletion,
            Kernel::IdentityMailboxProof,
        ];
        assert_eq!(
            Kernel::in_package(Package::Identity).collect::<Vec<_>>(),
            identity
        );
        for path in [
            Package::Identity.manifest(),
            "services/cantos-identity/src/lib.rs",
            "services/cantos-identity/src/domain/mod.rs",
            "services/cantos-identity/src/domain/ids.rs",
            Kernel::IdentitySecurityVersion.source(),
        ] {
            assert_eq!(pick(&[path], false), identity, "{path}");
        }
        assert_eq!(
            pick(&[Kernel::IdentityCompletion.source()], false),
            [Kernel::IdentityCompletion, Kernel::IdentityMailboxProof]
        );
        for path in [
            "services/cantos-identity/src/domain/mailbox_proof.rs",
            "services/cantos-identity/src/domain/mailbox_proof/tests.rs",
            Kernel::IdentityMailboxProof.source(),
        ] {
            assert_eq!(
                pick(&[path], false),
                [Kernel::IdentityMailboxProof],
                "{path}"
            );
        }
        assert_eq!(pick(&["services/cantos-identity/README.md"], false), []);
        assert_eq!(Kernel::IdentitySecurityVersion.covers(), 6);
        assert_eq!(Kernel::IdentityCompletion.covers(), 10);
        assert_eq!(Kernel::IdentityMailboxProof.covers(), 16);
        for kernel in identity {
            assert!(kernel.unreachable().is_empty());
        }
    }

    #[test]
    fn forge_attempt_ledger_registration_is_exact() {
        let kernel = Kernel::ForgeAttemptLedger;
        assert_eq!(kernel.name(), "forge-attempt-ledger");
        assert_eq!(
            kernel.harness(),
            "domain::attempt::kani_proofs::attempt_ledger_traces_never_overspend_or_blindly_retry"
        );
        assert_eq!(
            kernel.source(),
            "services/cantos-forge-functions/src/domain/attempt.rs"
        );
        assert_eq!(kernel.covers(), 9);
        assert!(kernel.unreachable().is_empty());
        assert_eq!(Kernel::ALL.len(), 14);
        assert_eq!(Kernel::ALL.last(), Some(&kernel));
    }

    #[test]
    fn every_command_names_its_package_and_the_forge_one_names_cantos_forge() {
        let workspace = Workspace::discover().unwrap();
        for kernel in Kernel::ALL {
            let command = kernel.command(&workspace);
            let packages: Vec<_> = command
                .args
                .windows(2)
                .filter(|args| args[0] == "--package")
                .map(|args| args[1].as_str())
                .collect();
            assert_eq!(packages, [kernel.package().name()], "{}", kernel.name());
            assert_eq!(
                command_package(&command).unwrap(),
                kernel.package(),
                "{}",
                kernel.name()
            );
        }
        let forge = Kernel::ForgeAttemptLedger.command(&workspace);
        assert!(forge
            .args
            .windows(2)
            .any(|args| args == ["--package", "cantos-forge"]));
        assert!(!forge.args.iter().any(|arg| arg == "cantos-server"));
        assert!(forge
            .args
            .windows(2)
            .any(|args| args == ["--cover-count", "9"]));
        assert!(!forge.args.contains(&"--allow-unreachable".to_owned()));
        let api = Kernel::CatalogRevision.command(&workspace);
        assert!(api
            .args
            .windows(2)
            .any(|args| args == ["--package", "cantos-server"]));
    }

    #[test]
    fn a_proof_command_without_exactly_one_registered_package_is_refused() {
        let command = |args: &[&str]| {
            CommandSpec::new("python3", "/example/checkout").args(args.iter().copied())
        };
        assert_eq!(
            command_package(&command(&["--package", "cantos-forge"])).unwrap(),
            Package::Forge
        );
        for args in [
            &[][..],
            &["--package"][..],
            &["--package", "cantos-contracts"][..],
            &["--package", "cantos-server", "--package", "cantos-forge"][..],
            &["--package", "cantos-server", "--package", "cantos-server"][..],
        ] {
            assert!(command_package(&command(args)).is_err(), "{args:?}");
        }
    }

    #[test]
    fn configuration_candidates_require_the_workspace_and_only_the_owning_manifest() {
        let root = Path::new("/example/checkout");
        let home = Path::new("/separate/cargo-home");
        let api = configuration_candidates(root, Some(home), Package::Api);
        let forge = configuration_candidates(root, Some(home), Package::Forge);
        assert!(api.contains(&root.join("services/cantos-server/Cargo.toml")));
        assert!(!api.contains(&root.join("services/cantos-forge-functions/Cargo.toml")));
        assert!(forge.contains(&root.join("services/cantos-forge-functions/Cargo.toml")));
        assert!(!forge.contains(&root.join("services/cantos-server/Cargo.toml")));
        for candidates in [&api, &forge] {
            assert!(candidates.contains(&root.join("Cargo.toml")));
            assert!(candidates.contains(&home.join("config.toml")));
            assert!(candidates.contains(&root.join(".cargo/config")));
        }
        // Only the owning manifest differs; the ancestor and Cargo-home set is shared.
        let shared = |set: &BTreeSet<PathBuf>, manifest: &str| {
            set.iter()
                .filter(|path| **path != root.join(manifest))
                .cloned()
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(
            shared(&api, "services/cantos-server/Cargo.toml"),
            shared(&forge, "services/cantos-forge-functions/Cargo.toml")
        );
    }

    #[test]
    fn prepared_snapshots_are_per_package_and_shared_within_one_package() {
        let workspace = Workspace::discover().unwrap();
        let mut steps = crate::gate::steps(
            &[
                crate::gate::Gate::Kani(Kernel::CatalogRevision),
                crate::gate::Gate::Kani(Kernel::ForgeAttemptLedger),
                crate::gate::Gate::Kani(Kernel::WavPayload),
            ],
            &workspace,
        );
        prepare_configuration(&workspace, &mut steps).unwrap();
        let snapshot = |step: &Step| {
            step.commands[0]
                .args
                .windows(2)
                .find(|args| args[0] == "--approved-config")
                .map(|args| args[1].clone())
                .unwrap()
        };
        assert_eq!(snapshot(&steps[0]), snapshot(&steps[2]));
        assert_ne!(snapshot(&steps[0]), snapshot(&steps[1]));
        let approved = |step: &Step| -> BTreeMap<String, Option<String>> {
            serde_json::from_str(&snapshot(step)).unwrap()
        };
        let digest = |relative: &str| {
            Some(format!(
                "{:x}",
                Sha256::digest(std::fs::read(workspace.path(relative)).unwrap())
            ))
        };
        let key = |relative: &str| workspace.path(relative).to_str().unwrap().to_owned();
        let (api, forge) = (approved(&steps[0]), approved(&steps[1]));
        assert_eq!(api[&key("Cargo.toml")], digest("Cargo.toml"));
        assert_eq!(forge[&key("Cargo.toml")], digest("Cargo.toml"));
        assert_eq!(
            api[&key("services/cantos-server/Cargo.toml")],
            digest("services/cantos-server/Cargo.toml")
        );
        assert!(!api.contains_key(&key("services/cantos-forge-functions/Cargo.toml")));
        assert_eq!(
            forge[&key("services/cantos-forge-functions/Cargo.toml")],
            digest("services/cantos-forge-functions/Cargo.toml")
        );
        assert!(!forge.contains_key(&key("services/cantos-server/Cargo.toml")));
    }

    #[test]
    fn forge_selection_is_package_local_in_both_directions() {
        let forge = [Kernel::ForgeAttemptLedger];
        for path in [
            "services/cantos-forge-functions/src/domain/attempt.rs",
            "services/cantos-forge-functions/src/domain/mod.rs",
            "services/cantos-forge-functions/src/domain/digest.rs",
            "services/cantos-forge-functions/Cargo.toml",
            "services/cantos-forge-functions/src/lib.rs",
        ] {
            assert_eq!(pick(&[path], false), forge, "{path}");
        }
        // Forge code that no registered proof reaches, and unrelated trees, select nothing.
        for path in [
            "services/cantos-forge-functions/src/application/mod.rs",
            "services/cantos-forge-functions/src/main.rs",
            "services/cantos-forge-functions/tests/cli.rs",
            "services/cantos-forge-functions/README.md",
            "services/cantos-forge-functions/src/domainx/attempt.rs",
        ] {
            assert_eq!(pick(&[path], false), [], "{path}");
        }
        // The shared primitive reaches both packages; uncertainty still selects everything.
        for path in [SCRIPT, "scripts/test_verify_kani.py"] {
            assert!(pick(&[path], false).contains(&Kernel::ForgeAttemptLedger));
        }
        assert_eq!(pick(&["Cargo.lock"], true), Kernel::ALL);
        assert_eq!(
            pick(
                &["services/cantos-forge-functions/src/domain/attempt.rs"],
                true
            ),
            Kernel::ALL
        );
        // No API path selects the Forge kernel, and no Forge path selects an API kernel.
        let api_paths = [
            "services/cantos-server/Cargo.toml",
            "services/cantos-server/build.rs",
            "services/cantos-server/src/lib.rs",
            "services/cantos-server/src/contexts/mod.rs",
            "services/cantos-server/src/contexts/media/mod.rs",
            "services/cantos-server/src/contexts/media/domain/mod.rs",
            "services/cantos-server/src/contexts/catalog/domain/error.rs",
            "services/cantos-server/src/contexts/listening/domain/mod.rs",
            "services/cantos-server/src/contract/studio/mod.rs",
            "services/cantos-server/contracts/studio/script-ir/0.1.0/script-ir.schema.json",
        ];
        for path in api_paths {
            assert!(
                !pick(&[path], false).contains(&Kernel::ForgeAttemptLedger),
                "{path}"
            );
        }
        for kernel in API_KERNELS {
            assert!(!pick(&[kernel.source()], false).contains(&Kernel::ForgeAttemptLedger));
        }
        assert_eq!(pick(&api_paths, false).last(), Some(&Kernel::PublicCover));
        let mixed = pick(
            &[
                "services/cantos-forge-functions/src/domain/attempt.rs",
                Kernel::ImageBounds.source(),
            ],
            false,
        );
        assert_eq!(mixed, [Kernel::ImageBounds, Kernel::ForgeAttemptLedger]);
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
