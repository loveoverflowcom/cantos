//! `cargo xtask verify native-auth-qualification`: the finite #336 Linux qualification campaign
//! for the native auth candidate. It composes existing owners and adds no verifier: the behavior
//! primitive (model, then pure-core conformance), Cargo JSON build records, the native isolation
//! primitive with its typed receipt, and the registered Kani lane. Setup and builds run first;
//! every selected test then runs under the guard (empty `PATH`, finite environment, Linux network
//! namespace), so execution never builds, installs or reaches beyond loopback.
//!
//! Selection is a fixed allowlist of exact tests whose names are checked against their sources;
//! nothing is discovered by wildcard. The Server session target is an agreed interface
//! ([`SESSION_TARGET`]): absent, its layer is `NOT_RUN` and the full candidate is not evaluated;
//! present, it is mandatory and any build, listing or run failure is `FAIL`.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use anyhow::{bail, ensure, Context, Result};
use clap::Args;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    capture::{Capture, Captured, SystemCapture},
    command::{CommandSpec, Runner},
    gate::Gate,
    kani::{self, Kernel},
    native_isolation::{self, NativeIsolationArgs},
    pipeline,
    report::{Report, Status},
    tools::ToolProbe,
    workspace::Workspace,
};

#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct QualificationArgs {
    /// Also run the three registered Identity Kani kernels on the pinned bundle (the D1 lane).
    #[arg(long)]
    pub kani: bool,
    /// Where to write the sanitized JSON report; defaults under target/verification.
    #[arg(long, value_name = "PATH")]
    pub report: Option<PathBuf>,
    /// Print the fixed plan without probing tools, building or running anything.
    #[arg(long)]
    pub dry_run: bool,
}

/// The agreed Server session interface: a Cargo integration target, its exact test and feature.
pub const SESSION_TARGET: &str = "identity_auth_candidate";
pub const SESSION_TEST: &str = "real_identity_actor_session_journey";
pub const SESSION_FEATURE: &str = "turso-candidate";

const SERVER: &str = "cantos-server";
const IDENTITY: &str = "cantos-identity";
const IDENTITY_FEATURE: &str = "service";
const CAPABILITIES: [&str; 2] = [
    "cantos.identity.native-mailbox-proof",
    "cantos.identity.native-session-admission",
];
const KERNELS: [Kernel; 3] = [
    Kernel::IdentitySecurityVersion,
    Kernel::IdentityCompletion,
    Kernel::IdentityMailboxProof,
];
/// The guard's own maximum test deadline; the journeys finish well inside it.
const TEST_TIMEOUT_SECONDS: u64 = 120;
const BUILD_TIMEOUT: Duration = Duration::from_secs(3600);
const GUARD_TIMEOUT: Duration = Duration::from_secs(300);
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// Each layer's status and its one-line detail.
type Layers = BTreeMap<Layer, (Status, String)>;
/// A built test executable's digest and path, or why the build or resolution failed.
type Built = Result<(String, PathBuf), String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Model,
    PureConformance,
    IdentityProcessRpc,
    ActorTurso,
    ServerSessionCandidate,
    Isolation,
    Kani,
    FullCandidate,
    FullServerHttp,
    Browser,
    Device,
}

impl Layer {
    pub const ALL: [Layer; 11] = [
        Layer::Model,
        Layer::PureConformance,
        Layer::IdentityProcessRpc,
        Layer::ActorTurso,
        Layer::ServerSessionCandidate,
        Layer::Isolation,
        Layer::Kani,
        Layer::FullCandidate,
        Layer::FullServerHttp,
        Layer::Browser,
        Layer::Device,
    ];

    /// Every layer the full candidate needs, each on the same source revision.
    const REQUIRED: [Layer; 7] = [
        Layer::Model,
        Layer::PureConformance,
        Layer::IdentityProcessRpc,
        Layer::ActorTurso,
        Layer::ServerSessionCandidate,
        Layer::Isolation,
        Layer::Kani,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Layer::Model => "MODEL",
            Layer::PureConformance => "PURE_CONFORMANCE",
            Layer::IdentityProcessRpc => "IDENTITY_PROCESS_RPC",
            Layer::ActorTurso => "ACTOR_TURSO",
            Layer::ServerSessionCandidate => "SERVER_SESSION_CANDIDATE",
            Layer::Isolation => "ISOLATION",
            Layer::Kani => "KANI",
            Layer::FullCandidate => "FULL_CANDIDATE",
            Layer::FullServerHttp => "FULL_SERVER_HTTP",
            Layer::Browser => "BROWSER",
            Layer::Device => "DEVICE",
        }
    }
}

/// Which compiled test executable owns a selected test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Unit {
    Lib,
    Test(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Selected {
    layer: Layer,
    package: &'static str,
    feature: &'static str,
    unit: Unit,
    test: &'static str,
    /// Starts the real Identity executable, which the guard then binds by digest.
    child: bool,
    /// The file whose `fn` the exact name's last segment must name.
    source: &'static str,
}

const fn actor(test: &'static str) -> Selected {
    Selected {
        layer: Layer::ActorTurso,
        package: SERVER,
        feature: SESSION_FEATURE,
        unit: Unit::Lib,
        test,
        child: false,
        source: "services/cantos-server/src/platform/identity/turso/tests.rs",
    }
}

const fn session(test: &'static str) -> Selected {
    Selected {
        layer: Layer::ServerSessionCandidate,
        package: SERVER,
        feature: SESSION_FEATURE,
        unit: Unit::Lib,
        test,
        child: false,
        source: "services/cantos-server/src/platform/identity_auth/turso/tests.rs",
    }
}

const fn identity(test: &'static str) -> Selected {
    Selected {
        layer: Layer::IdentityProcessRpc,
        package: IDENTITY,
        feature: IDENTITY_FEATURE,
        unit: Unit::Test("process"),
        test,
        child: true,
        source: "services/cantos-identity/tests/process.rs",
    }
}

/// The finite allowlist. Race, reopen and unknown-acknowledgement regressions protect the
/// layers' claims beyond their happy paths.
const SELECTED: [Selected; 12] = [
    identity("mailbox_owner_signs_up_then_authenticates_again_after_a_restart"),
    identity("a_caller_that_is_not_an_allowed_peer_gets_nothing_and_causes_no_effect"),
    actor("platform::identity::turso::tests::registration_lookup_and_linking_follow_the_port_contract_on_a_real_file"),
    actor("platform::identity::turso::tests::concurrent_registrations_of_one_key_commit_one_binding_and_no_losing_actor"),
    actor("platform::identity::turso::tests::bindings_survive_reopen_and_replay_returns_the_original_actor"),
    actor("platform::identity::turso::tests::a_lost_acknowledgement_after_a_real_commit_reconciles_by_exact_key_replay"),
    Selected {
        layer: Layer::ServerSessionCandidate,
        package: SERVER,
        feature: SESSION_FEATURE,
        unit: Unit::Test(SESSION_TARGET),
        test: SESSION_TEST,
        child: true,
        source: "services/cantos-server/tests/identity_auth_candidate.rs",
    },
    session("platform::identity_auth::turso::tests::concurrent_refreshes_spend_once_and_the_loser_revokes_the_family"),
    session("platform::identity_auth::turso::tests::a_lost_acknowledgement_is_unknown_and_reconciles_without_a_retry"),
    session("platform::identity_auth::turso::tests::revocation_is_idempotent_keeps_its_first_reason_and_survives_reopen"),
    session("platform::identity_auth::turso::tests::every_security_change_revokes_the_turso_family_durably_and_never_resurrects"),
    session("platform::identity_auth::turso::tests::concurrent_first_logins_of_one_subject_bind_exactly_one_actor"),
];

const LIMITATIONS: [&str; 8] = [
    "the candidate journey runs in a test process that owns server.db; it is not the production Server binary, HTTP router or cookie transport",
    "model simulation, pure-core observers, file-backed Turso tests and real Identity RPC are separate evidence layers",
    "an artifact digest proves which file ran and that it stayed unchanged; the build record, not the guard, ties it to the source revision",
    "the guard is not a filesystem or syscall sandbox; absolute paths remain reachable",
    "the Server decisions decide_issue/decide_admission have no Kani kernel compatible with the fixed unwind 2 lane; KANI covers only the three registered Identity kernels",
    "the inventory mismatches (wrong-subject reply, failed revoke write, Studio roster remove/re-add) remain open and are not resolved by any layer",
    "full Server HTTP, browser and device acceptance have no lane in this campaign",
    "no deployment, migration of live data, credential or provider call is made",
];

// ---- pure decisions over tool output --------------------------------------------------------

#[derive(Deserialize)]
struct Message {
    reason: String,
    #[serde(default)]
    package_id: String,
    target: Option<MessageTarget>,
    profile: Option<MessageProfile>,
    executable: Option<PathBuf>,
}

#[derive(Deserialize)]
struct MessageTarget {
    name: String,
    kind: Vec<String>,
}

#[derive(Deserialize)]
struct MessageProfile {
    test: bool,
}

/// What a Cargo build must have produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Artifact {
    LibTest,
    Test(&'static str),
    Bin(&'static str),
}

/// The one executable Cargo reported for exactly this package id and artifact. Older files in
/// the target directory are never considered: only this invocation's messages are read.
fn executable(output: &str, package_id: &str, artifact: Artifact) -> Result<PathBuf> {
    let mut found = Vec::new();
    for line in output.lines() {
        let Ok(message) = serde_json::from_str::<Message>(line) else {
            continue;
        };
        if message.reason != "compiler-artifact" || message.package_id != package_id {
            continue;
        }
        let (Some(target), Some(profile), Some(path)) =
            (message.target, message.profile, message.executable)
        else {
            continue;
        };
        let kind = |expected: &str| target.kind.len() == 1 && target.kind[0] == expected;
        let matches = match artifact {
            Artifact::LibTest => kind("lib") && profile.test,
            Artifact::Test(name) => kind("test") && target.name == name && profile.test,
            Artifact::Bin(name) => kind("bin") && target.name == name && !profile.test,
        };
        if matches {
            found.push(path);
        }
    }
    ensure!(
        found.len() == 1,
        "Cargo reported {} executables for {artifact:?}",
        found.len()
    );
    let path = found.remove(0);
    ensure!(path.is_absolute(), "Cargo reported a relative executable");
    Ok(path)
}

/// The workspace package ids by name, from `cargo metadata --no-deps`.
fn package_ids(metadata: &Value) -> Result<BTreeMap<String, String>> {
    let packages = metadata["packages"]
        .as_array()
        .context("metadata lists no packages")?;
    let mut ids = BTreeMap::new();
    for package in packages {
        let (Some(name), Some(id)) = (package["name"].as_str(), package["id"].as_str()) else {
            bail!("metadata package without a name or id");
        };
        ensure!(
            ids.insert(name.to_owned(), id.to_owned()).is_none(),
            "duplicate package {name}"
        );
    }
    Ok(ids)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Presence {
    Absent,
    Present,
}

/// Whether the Server owner has added the agreed session target. A target of that name with any
/// other shape is a broken interface, never "absent".
fn session_target(metadata: &Value) -> Result<Presence> {
    let server = metadata["packages"]
        .as_array()
        .and_then(|packages| packages.iter().find(|p| p["name"] == SERVER))
        .context("metadata has no cantos-server package")?;
    let targets: Vec<&Value> = server["targets"]
        .as_array()
        .context("cantos-server has no target list")?
        .iter()
        .filter(|target| target["name"] == SESSION_TARGET)
        .collect();
    match targets.as_slice() {
        [] => Ok(Presence::Absent),
        [target]
            if target["kind"] == json!(["test"])
                && target["required-features"] == json!([SESSION_FEATURE]) =>
        {
            Ok(Presence::Present)
        }
        _ => {
            bail!("{SESSION_TARGET} exists but is not one test target requiring {SESSION_FEATURE}")
        }
    }
}

/// libtest `--list --exact NAME` output must name the test exactly once.
fn listed_once(output: &str, test: &str) -> Result<()> {
    let expected = format!("{test}: test");
    let count = output.lines().filter(|line| *line == expected).count();
    ensure!(count == 1, "{test} is listed {count} times");
    Ok(())
}

/// The status a selected test earns from a validated guard receipt, and the guard's own status.
/// macOS-style `UNSUPPORTED` with one passing test is a passed test without isolation.
fn test_status(guard: Status, tests_passed: usize) -> Status {
    match guard {
        Status::Pass => Status::Pass,
        Status::Unsupported if tests_passed == 1 => Status::Pass,
        other => other,
    }
}

/// Every member must pass for the aggregate to pass; any failure dominates, then unsupported.
fn aggregate(statuses: impl IntoIterator<Item = Status>) -> Status {
    let statuses: Vec<Status> = statuses.into_iter().collect();
    if statuses.is_empty() {
        Status::NotRun
    } else if statuses.contains(&Status::Fail) {
        Status::Fail
    } else if statuses.contains(&Status::Unsupported) {
        Status::Unsupported
    } else if statuses.iter().all(|status| *status == Status::Pass) {
        Status::Pass
    } else {
        Status::NotRun
    }
}

/// Whether the campaign's evidence belongs to one committed source. `unchanged` covers the
/// revision, status listing and lockfile before and after, and one raw-content snapshot across
/// every guard receipt; `clean` means neither Git nor any receipt saw uncommitted content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Provenance {
    unchanged: bool,
    clean: bool,
}

/// The full candidate is evaluated only when every required layer has evidence on one unchanged,
/// committed source revision; a missing layer or a dirty tree leaves it `NOT_RUN`, never `PASS`.
fn full_candidate(layers: &Layers, provenance: Provenance) -> (Status, String) {
    if !provenance.unchanged {
        return (
            Status::Fail,
            "source content or lockfile changed during the campaign".into(),
        );
    }
    let status = |layer: &Layer| layers.get(layer).map(|(status, _)| *status);
    let failed: Vec<&str> = Layer::REQUIRED
        .iter()
        .filter(|layer| status(layer) == Some(Status::Fail))
        .map(|layer| layer.name())
        .collect();
    let missing: Vec<&str> = Layer::REQUIRED
        .iter()
        .filter(|layer| status(layer) != Some(Status::Pass))
        .map(|layer| layer.name())
        .collect();
    if !failed.is_empty() {
        (
            Status::Fail,
            format!("required layer failed: {}", failed.join(", ")),
        )
    } else if !missing.is_empty() {
        (
            Status::NotRun,
            format!("not evaluated; without evidence: {}", missing.join(", ")),
        )
    } else if !provenance.clean {
        (
            Status::NotRun,
            "not evaluated: the tree has uncommitted content; qualify a committed revision".into(),
        )
    } else {
        (
            Status::Pass,
            "every required layer passed on one committed source revision".into(),
        )
    }
}

/// Copy the built Identity executable to a campaign-owned path, so a later build of the same
/// package (test builds re-link `target/debug/cantos-identity` with dev-dependency features)
/// cannot replace the file the receipts bind. The copy must hash exactly like the build output.
fn retain(built: &Path, destination: &Path) -> Result<(String, PathBuf)> {
    let digest = sha256_file(built)?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::remove_file(destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    std::fs::copy(built, destination)?;
    ensure!(
        sha256_file(destination)? == digest,
        "the retained Identity executable differs from the build output"
    );
    Ok((digest, destination.to_path_buf()))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

// ---- the plan ---------------------------------------------------------------------------------

fn cargo(workspace: &Workspace, args: &[&str]) -> CommandSpec {
    CommandSpec::new("cargo", workspace.root()).args(args.iter().copied())
}

/// One Cargo invocation per compiled test executable group, so a broken session target never
/// hides the Actor Registry layer.
fn build_command(workspace: &Workspace, package: &str, feature: &str, unit: Unit) -> CommandSpec {
    let mut args = vec!["test", "--locked", "-p", package, "--features", feature];
    match unit {
        Unit::Lib => args.push("--lib"),
        Unit::Test(name) => args.extend(["--test", name]),
    }
    args.extend(["--no-run", "--message-format=json-render-diagnostics"]);
    cargo(workspace, &args)
}

fn identity_build(workspace: &Workspace) -> CommandSpec {
    cargo(
        workspace,
        &[
            "build",
            "--locked",
            "-p",
            IDENTITY,
            "--features",
            IDENTITY_FEATURE,
            "--bin",
            IDENTITY,
            "--message-format=json-render-diagnostics",
        ],
    )
}

fn groups() -> Vec<(&'static str, &'static str, Unit)> {
    let mut groups: Vec<_> = SELECTED
        .iter()
        .map(|selected| (selected.package, selected.feature, selected.unit))
        .collect();
    groups.sort();
    groups.dedup();
    groups
}

fn plan(workspace: &Workspace) -> Value {
    let root = workspace.root();
    let mut builds = vec![identity_build(workspace).display(root)];
    builds.extend(groups().into_iter().map(|(package, feature, unit)| {
        build_command(workspace, package, feature, unit).display(root)
    }));
    json!({
        "schema_version": 1, "status": "NOT_RUN", "dry_run": true,
        "steps": [
            "record source revision, lockfile digest and tool identities",
            "read cargo metadata: the Server session target is required when present and NOT_RUN when absent",
            "build every artifact first and resolve each executable from this invocation's Cargo JSON",
            "retain the Identity executable at a campaign-owned path before any test build can re-link it",
            "list each exact test once, then run it under verify native-isolation (Identity child bound by digest)",
            "run the behavior model, then pure-core conformance, for each native auth capability",
            "with --kani: run the three registered Identity kernels on the pinned bundle",
            "recheck the source revision and lockfile, then write one sanitized report",
        ],
        "builds": builds,
        "tests": SELECTED.iter().map(|selected| json!({
            "layer": selected.layer.name(), "package": selected.package, "test": selected.test,
            "identity_child": selected.child,
        })).collect::<Vec<_>>(),
        "capabilities": CAPABILITIES,
        "kernels": KERNELS.map(Kernel::name),
        "layers": Layer::ALL.map(Layer::name),
        "limitations": LIMITATIONS,
    })
}

// ---- execution --------------------------------------------------------------------------------

struct Campaign<'a> {
    workspace: &'a Workspace,
    capture: &'a mut dyn Capture,
    tools: &'a dyn ToolProbe,
    runner: &'a mut dyn Runner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    sha: String,
    dirty: bool,
    status_sha256: String,
    lock_sha256: String,
}

impl Campaign<'_> {
    fn captured(&mut self, spec: &CommandSpec, timeout: Duration) -> Result<Captured> {
        self.capture
            .capture(spec, timeout)
            .map_err(|error| anyhow::anyhow!("{} {error}", spec.program))
    }

    fn line(&mut self, program: &str, args: &[&str]) -> String {
        let spec = CommandSpec::new(program, self.workspace.root()).args(args.iter().copied());
        match self.captured(&spec, PROBE_TIMEOUT) {
            Ok(out) if out.code == 0 => out
                .stdout
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned(),
            _ => "unavailable".into(),
        }
    }

    fn source(&mut self) -> Result<Source> {
        let root = self.workspace.root().to_path_buf();
        let git = |args: &[&str]| CommandSpec::new("git", &root).args(args.iter().copied());
        let head = self.captured(&git(&["rev-parse", "HEAD"]), PROBE_TIMEOUT)?;
        let status = self.captured(
            &git(&["status", "--porcelain=v1", "--untracked-files=all"]),
            PROBE_TIMEOUT,
        )?;
        ensure!(
            head.code == 0 && status.code == 0,
            "git source identity failed"
        );
        let sha = head.stdout.trim().to_owned();
        ensure!(
            sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid source revision"
        );
        Ok(Source {
            sha,
            dirty: !status.stdout.trim().is_empty(),
            status_sha256: format!("{:x}", Sha256::digest(status.stdout.as_bytes())),
            lock_sha256: sha256_file(&self.workspace.path("Cargo.lock"))?,
        })
    }

    fn build(&mut self, spec: &CommandSpec) -> Result<String> {
        let out = self.captured(spec, BUILD_TIMEOUT)?;
        if out.code != 0 {
            // Compiler diagnostics are not secret and are the only way to debug a CI build.
            eprintln!("{}", out.stderr);
            bail!("build exited with {}", out.code);
        }
        Ok(out.stdout)
    }

    fn behavior(&mut self, conformance: bool, capability: &str) -> Status {
        let step = Gate::Behavior {
            capability: Some(capability.to_owned()),
            setup: false,
            conformance,
        }
        .step(self.workspace);
        let report = pipeline::execute("behavior", &[step], self.tools, self.runner);
        report.outcomes.first().map_or(Status::NotRun, |o| o.status)
    }

    fn kani(&mut self) -> (Status, String) {
        let mut steps: Vec<_> = KERNELS
            .iter()
            .map(|kernel| Gate::Kani(*kernel).step(self.workspace))
            .collect();
        if let Err(error) = kani::prepare_configuration(self.workspace, &mut steps) {
            return (
                Status::Fail,
                format!("proof configuration refused: {error:#}"),
            );
        }
        let report = pipeline::execute("kani", &steps, self.tools, self.runner);
        let detail = report
            .outcomes
            .iter()
            .map(|o| format!("{} {}", o.name, o.status))
            .collect::<Vec<_>>()
            .join("; ");
        (aggregate(report.outcomes.iter().map(|o| o.status)), detail)
    }
}

struct TestRun {
    selected: Selected,
    status: Status,
    guard: Option<Status>,
    detail: String,
    receipt: Option<Value>,
    /// The guard's raw-content snapshot and dirty flag, for one-source provenance.
    content: Option<(String, bool)>,
}

/// Run one listed exact test under the guard and check that its receipt binds the built files.
fn guarded(
    campaign: &mut Campaign<'_>,
    selected: Selected,
    executable: &Path,
    digest: &str,
    child: Option<(&Path, &str)>,
    source: &Source,
) -> TestRun {
    let failed = |detail: String| TestRun {
        selected,
        status: Status::Fail,
        guard: None,
        detail,
        receipt: None,
        content: None,
    };
    let list = CommandSpec::new(executable.to_string_lossy(), campaign.workspace.root()).args([
        selected.test,
        "--exact",
        "--list",
    ]);
    match campaign.captured(&list, PROBE_TIMEOUT) {
        Ok(out) if out.code == 0 => {
            if let Err(error) = listed_once(&out.stdout, selected.test) {
                return failed(format!("{error:#}"));
            }
        }
        _ => return failed("listing the exact test failed".into()),
    }
    let args = NativeIsolationArgs {
        test_binary: executable.to_path_buf(),
        test: selected.test.to_owned(),
        timeout_seconds: TEST_TIMEOUT_SECONDS,
        identity_binary: child.map(|(path, _)| path.to_path_buf()),
        dry_run: false,
    };
    let spec = native_isolation::command(campaign.workspace, &args);
    let Ok(out) = campaign.captured(&spec, GUARD_TIMEOUT) else {
        return failed(
            "isolation primitive failed or exceeded its bound; raw output suppressed".into(),
        );
    };
    let Ok((outcome, guard)) = native_isolation::outcome_status(
        &out.stdout,
        out.code,
        selected.test,
        TEST_TIMEOUT_SECONDS,
        child.is_some(),
    ) else {
        return failed("invalid or inconsistent isolation receipt; raw output suppressed".into());
    };
    let receipt = serde_json::to_value(&outcome).ok();
    let ran = outcome.tests_passed() == 1;
    let bound = !ran
        || (outcome.binary_sha256() == Some(digest)
            && outcome.identity_binary_sha256() == child.map(|(_, digest)| digest)
            && outcome.source().map(|s| s.sha.as_str()) == Some(source.sha.as_str()));
    if !bound {
        return TestRun {
            receipt,
            ..failed("receipt does not bind the built artifacts and source revision".into())
        };
    }
    let status = test_status(guard, outcome.tests_passed());
    // A test that passed while the guard could not isolate it is a passed, unisolated test.
    let detail = if status == Status::Pass && guard != Status::Pass {
        format!("passed without isolation: {}", outcome.reason())
    } else {
        outcome.reason().to_owned()
    };
    TestRun {
        selected,
        status,
        guard: Some(guard),
        detail,
        receipt,
        content: outcome
            .source()
            .map(|source| (source.diff_sha256.clone(), source.dirty)),
    }
}

pub fn execute(
    workspace: &Workspace,
    args: &QualificationArgs,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    if args.dry_run {
        println!("{}", serde_json::to_string_pretty(&plan(workspace))?);
        return Ok(ExitCode::SUCCESS);
    }
    let mut capture = SystemCapture::new()?.with_cleanup_grace(Duration::from_secs(10));
    let mut campaign = Campaign {
        workspace,
        capture: &mut capture,
        tools,
        runner,
    };
    let (layers, document) = run(&mut campaign, args.kani)?;
    let path = args.report.clone().unwrap_or_else(|| {
        workspace.path("target/verification/native-auth-qualification/report.json")
    });
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&document)? + "\n")?;
    let mut report = Report::new("verify native-auth-qualification");
    for (layer, (status, detail)) in &layers {
        report.record(layer.name(), *status, detail.clone());
    }
    report.note(format!("sanitized report: {}", path.display()));
    // The D1 run exists to qualify the full candidate: anything short of it is not green.
    let full = layers.get(&Layer::FullCandidate).map(|(status, _)| *status);
    if args.kani && full != Some(Status::Pass) {
        report.note("--kani (the D1 lane) requires FULL_CANDIDATE PASS; exiting with failure");
        report.print();
        return Ok(ExitCode::FAILURE);
    }
    report.print();
    Ok(report.exit_code())
}

fn run(campaign: &mut Campaign<'_>, kani: bool) -> Result<(Layers, Value)> {
    let before = campaign.source()?;
    let tools = json!({
        "rustc": campaign.line("rustc", &["-V"]),
        "cargo": campaign.line("cargo", &["-V"]),
        "python3": campaign.line("python3", &["--version"]),
        "git": campaign.line("git", &["--version"]),
        "unshare": campaign.line("unshare", &["--version"]),
        "behavior": campaign.line("python3", &["scripts/verify_behavior.py", "--probe"]),
        "kani_model": {"kani": kani::KANI_VERSION, "cbmc": kani::CBMC_VERSION,
                       "compiler": kani::RUSTC_VERSION, "toolchain": kani::TOOLCHAIN, "target": kani::TARGET},
        "host": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
    });
    let mut layers = Layers::new();
    let mut builds = Vec::new();
    let root = campaign.workspace.root().to_path_buf();

    let metadata_spec = cargo(
        campaign.workspace,
        &["metadata", "--format-version", "1", "--no-deps", "--locked"],
    );
    let metadata: Value = serde_json::from_str(&campaign.build(&metadata_spec)?)?;
    let ids = package_ids(&metadata)?;
    let id = |package: &str| {
        ids.get(package)
            .cloned()
            .with_context(|| format!("metadata has no {package}"))
    };
    let identity_id = id(IDENTITY)?;
    let server_id = id(SERVER)?;
    let presence = session_target(&metadata);

    // Builds come first; nothing below compiles.
    let identity_spec = identity_build(campaign.workspace);
    let retained = campaign
        .workspace
        .path("target/verification/native-auth-qualification/artifacts/cantos-identity");
    let identity_binary = campaign
        .build(&identity_spec)
        .and_then(|out| executable(&out, &identity_id, Artifact::Bin(IDENTITY)))
        .and_then(|path| retain(&path, &retained));
    builds.push(json!({
        "argv": identity_spec.display(&root),
        "status": if identity_binary.is_ok() { "PASS" } else { "FAIL" },
        "artifacts": identity_binary.as_ref().map(|(digest, _)| vec![json!({"role": "identity-executable", "sha256": digest})]).unwrap_or_default(),
    }));
    let mut executables: BTreeMap<(&str, &str, Unit), Built> = BTreeMap::new();
    for (package, feature, unit) in groups() {
        if matches!(unit, Unit::Test(SESSION_TARGET))
            && presence.as_ref().ok() != Some(&Presence::Present)
        {
            continue;
        }
        let spec = build_command(campaign.workspace, package, feature, unit);
        let artifact = match unit {
            Unit::Lib => Artifact::LibTest,
            Unit::Test(name) => Artifact::Test(name),
        };
        let package_id = if package == IDENTITY {
            &identity_id
        } else {
            &server_id
        };
        let built = campaign
            .build(&spec)
            .and_then(|out| executable(&out, package_id, artifact))
            .and_then(|path| Ok((sha256_file(&path)?, path)))
            .map_err(|error| format!("{error:#}"));
        builds.push(json!({
            "argv": spec.display(&root),
            "status": if built.is_ok() { "PASS" } else { "FAIL" },
            "artifacts": built.as_ref().map(|(digest, _)| vec![json!({"role": format!("{package} {unit:?}"), "sha256": digest})]).unwrap_or_default(),
        }));
        executables.insert((package, feature, unit), built);
    }

    // Exact tests under the guard.
    let mut runs = Vec::new();
    for selected in SELECTED {
        if selected.layer == Layer::ServerSessionCandidate {
            match &presence {
                Ok(Presence::Present) => {}
                Ok(Presence::Absent) => continue,
                Err(error) => {
                    runs.push(TestRun {
                        selected,
                        status: Status::Fail,
                        guard: None,
                        detail: format!("{error:#}"),
                        receipt: None,
                        content: None,
                    });
                    continue;
                }
            }
        }
        let built = &executables[&(selected.package, selected.feature, selected.unit)];
        let (digest, path) = match built {
            Ok(value) => value.clone(),
            Err(error) => {
                runs.push(TestRun {
                    selected,
                    status: Status::Fail,
                    guard: None,
                    detail: format!("build failed: {error}"),
                    receipt: None,
                    content: None,
                });
                continue;
            }
        };
        let child = if selected.child {
            match &identity_binary {
                Ok((digest, path)) => Some((path.as_path(), digest.as_str())),
                Err(error) => {
                    runs.push(TestRun {
                        selected,
                        status: Status::Fail,
                        guard: None,
                        detail: format!("Identity build failed: {error:#}"),
                        receipt: None,
                        content: None,
                    });
                    continue;
                }
            }
        } else {
            None
        };
        runs.push(guarded(campaign, selected, &path, &digest, child, &before));
    }
    for layer in [
        Layer::IdentityProcessRpc,
        Layer::ActorTurso,
        Layer::ServerSessionCandidate,
    ] {
        let members: Vec<&TestRun> = runs
            .iter()
            .filter(|run| run.selected.layer == layer)
            .collect();
        let entry = if layer == Layer::ServerSessionCandidate
            && matches!(presence, Ok(Presence::Absent))
        {
            (
                Status::NotRun,
                format!("prerequisite absent: no {SESSION_TARGET} target with {SESSION_FEATURE}"),
            )
        } else {
            let status = aggregate(members.iter().map(|run| run.status));
            let problems: Vec<String> = members
                .iter()
                .filter(|run| run.status != Status::Pass)
                .map(|run| format!("{}: {}", run.selected.test, run.detail))
                .collect();
            let mut detail = format!("{} exact test(s)", members.len());
            if !problems.is_empty() {
                detail = format!("{detail}; {}", problems.join("; "));
            }
            (status, detail)
        };
        layers.insert(layer, entry);
    }
    let guards: Vec<Status> = runs
        .iter()
        .map(|run| run.guard.unwrap_or(Status::Fail))
        .collect();
    layers.insert(
        Layer::Isolation,
        (
            aggregate(guards.iter().copied()),
            format!("{} guarded run(s)", guards.len()),
        ),
    );

    // Behavior: the model first, then the pure-core observers over it.
    let models: Vec<Status> = CAPABILITIES
        .iter()
        .map(|cap| campaign.behavior(false, cap))
        .collect();
    let model = aggregate(models.iter().copied());
    layers.insert(Layer::Model, (model, CAPABILITIES.join(", ")));
    let conformance = if model == Status::Pass {
        let observed: Vec<Status> = CAPABILITIES
            .iter()
            .map(|cap| campaign.behavior(true, cap))
            .collect();
        (
            aggregate(observed),
            "registered pure-core observers with a disclosed synthetic shell".into(),
        )
    } else {
        (Status::NotRun, "the model layer did not pass".into())
    };
    layers.insert(Layer::PureConformance, conformance);
    layers.insert(
        Layer::Kani,
        if kani {
            campaign.kani()
        } else {
            (
                Status::NotRun,
                "not requested; pass --kani on the D1 Ubuntu x86_64 runner".into(),
            )
        },
    );

    let after = campaign.source()?;
    let snapshots: std::collections::BTreeSet<&str> = runs
        .iter()
        .filter_map(|run| run.content.as_ref().map(|(digest, _)| digest.as_str()))
        .collect();
    let provenance = Provenance {
        unchanged: before == after && snapshots.len() <= 1,
        clean: !before.dirty
            && !after.dirty
            && runs
                .iter()
                .all(|run| run.content.as_ref().is_some_and(|(_, dirty)| !dirty)),
    };
    let unchanged = provenance.unchanged;
    layers.insert(Layer::FullCandidate, full_candidate(&layers, provenance));
    for (layer, reason) in [
        (
            Layer::FullServerHttp,
            "no production Server binary or HTTP lane runs in this campaign",
        ),
        (Layer::Browser, "no browser lane runs in this campaign"),
        (
            Layer::Device,
            "no Android/iOS device lane runs in this campaign",
        ),
    ] {
        layers.insert(layer, (Status::NotRun, reason.into()));
    }

    let document = json!({
        "schema_version": 1,
        "campaign": "native-auth-qualification",
        "source": {"sha": before.sha, "dirty": before.dirty, "status_sha256": before.status_sha256,
                   "lock_sha256": before.lock_sha256, "unchanged": unchanged,
                   "clean": provenance.clean,
                   "content_sha256": snapshots.iter().next().copied()},
        "tools": tools,
        "builds": builds,
        "tests": runs.iter().map(|run| json!({
            "layer": run.selected.layer.name(), "package": run.selected.package,
            "test": run.selected.test, "identity_child": run.selected.child,
            "listed": run.guard.is_some(), "status": run.status.to_string(),
            "guard": run.guard.map(|status| status.to_string()),
            "detail": run.detail, "receipt": run.receipt,
        })).collect::<Vec<_>>(),
        "layers": layers.iter().map(|(layer, (status, detail))| json!({
            "layer": layer.name(), "status": status.to_string(), "detail": detail,
        })).collect::<Vec<_>>(),
        "limitations": LIMITATIONS,
    });
    Ok((layers, document))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(package: &str, name: &str, kind: &str, test: bool, executable: &str) -> String {
        json!({"reason": "compiler-artifact", "package_id": package,
               "target": {"name": name, "kind": [kind]}, "profile": {"test": test},
               "executable": executable})
        .to_string()
    }

    const SERVER_ID: &str = "path+file:///repo/services/cantos-server#0.1.0";

    #[test]
    fn executables_come_only_from_this_invocation_for_the_exact_package_and_artifact() {
        let output = [
            message(
                SERVER_ID,
                "cantos_server",
                "lib",
                true,
                "/repo/target/debug/deps/lib-a",
            ),
            message(
                SERVER_ID,
                "cantos_server",
                "lib",
                false,
                "/repo/target/debug/deps/plain",
            ),
            message(
                SERVER_ID,
                SESSION_TARGET,
                "test",
                true,
                "/repo/target/debug/deps/journey-b",
            ),
            message(
                "path+file:///repo/services/cantos-identity#0.1.0",
                "process",
                "test",
                true,
                "/x/process",
            ),
            r#"{"reason":"build-finished","success":true}"#.to_owned(),
            "not json".to_owned(),
        ]
        .join("\n");
        assert_eq!(
            executable(&output, SERVER_ID, Artifact::LibTest).unwrap(),
            PathBuf::from("/repo/target/debug/deps/lib-a")
        );
        assert_eq!(
            executable(&output, SERVER_ID, Artifact::Test(SESSION_TARGET)).unwrap(),
            PathBuf::from("/repo/target/debug/deps/journey-b")
        );
        // Zero, two, another package's or a relative executable never resolves.
        assert!(executable(&output, SERVER_ID, Artifact::Test("process")).is_err());
        assert!(executable(&output, SERVER_ID, Artifact::Bin("cantos-server")).is_err());
        let doubled = format!(
            "{output}\n{}",
            message(SERVER_ID, "cantos_server", "lib", true, "/old")
        );
        assert!(executable(&doubled, SERVER_ID, Artifact::LibTest).is_err());
        let relative = message(SERVER_ID, "cantos-identity", "bin", false, "target/debug/x");
        assert!(executable(&relative, SERVER_ID, Artifact::Bin("cantos-identity")).is_err());
    }

    fn metadata(targets: Value) -> Value {
        json!({"packages": [{"name": SERVER, "id": SERVER_ID, "targets": targets},
                            {"name": IDENTITY, "id": "identity-id", "targets": []}]})
    }

    #[test]
    fn the_session_target_is_absent_present_or_a_broken_interface() {
        let lib = json!({"name": "cantos_server", "kind": ["lib"], "required-features": null});
        assert_eq!(
            session_target(&metadata(json!([lib]))).unwrap(),
            Presence::Absent
        );
        let target = json!({"name": SESSION_TARGET, "kind": ["test"], "required-features": [SESSION_FEATURE]});
        assert_eq!(
            session_target(&metadata(json!([lib, target]))).unwrap(),
            Presence::Present
        );
        for broken in [
            json!({"name": SESSION_TARGET, "kind": ["test"], "required-features": null}),
            json!({"name": SESSION_TARGET, "kind": ["bin"], "required-features": [SESSION_FEATURE]}),
            json!({"name": SESSION_TARGET, "kind": ["test"], "required-features": [SESSION_FEATURE, "dbsp-experimental"]}),
        ] {
            assert!(
                session_target(&metadata(json!([lib, broken]))).is_err(),
                "{broken}"
            );
        }
        assert!(session_target(&metadata(json!([target.clone(), target]))).is_err());
        assert_eq!(
            package_ids(&metadata(json!([]))).unwrap()[SERVER],
            SERVER_ID
        );
    }

    #[test]
    fn an_exact_test_must_be_listed_exactly_once() {
        let test = "platform::x::tests::exact";
        listed_once(&format!("{test}: test\n\n1 test, 0 benchmarks\n"), test).unwrap();
        for output in [
            "\n0 tests, 0 benchmarks\n".to_owned(),
            format!("{test}: test\n{test}: test\n"),
            format!("{test}_longer: test\n"),
            format!("{test}: bench\n"),
        ] {
            assert!(listed_once(&output, test).is_err(), "{output}");
        }
    }

    #[test]
    fn a_guard_status_maps_to_test_and_layer_outcomes_without_promotion() {
        assert_eq!(test_status(Status::Pass, 1), Status::Pass);
        assert_eq!(test_status(Status::Unsupported, 1), Status::Pass);
        assert_eq!(test_status(Status::Unsupported, 0), Status::Unsupported);
        assert_eq!(test_status(Status::Fail, 0), Status::Fail);
        assert_eq!(aggregate([]), Status::NotRun);
        assert_eq!(aggregate([Status::Pass, Status::Pass]), Status::Pass);
        assert_eq!(aggregate([Status::Pass, Status::NotRun]), Status::NotRun);
        assert_eq!(
            aggregate([Status::Unsupported, Status::Pass]),
            Status::Unsupported
        );
        assert_eq!(aggregate([Status::Unsupported, Status::Fail]), Status::Fail);
    }

    fn layers(status: impl Fn(Layer) -> Status) -> Layers {
        Layer::REQUIRED
            .iter()
            .map(|layer| (*layer, (status(*layer), String::new())))
            .collect()
    }

    #[test]
    fn the_full_candidate_needs_every_required_layer_on_one_committed_revision() {
        let committed = Provenance {
            unchanged: true,
            clean: true,
        };
        assert_eq!(
            full_candidate(&layers(|_| Status::Pass), committed).0,
            Status::Pass
        );
        let changed = Provenance {
            unchanged: false,
            clean: true,
        };
        assert_eq!(
            full_candidate(&layers(|_| Status::Pass), changed).0,
            Status::Fail
        );
        let dirty = Provenance {
            unchanged: true,
            clean: false,
        };
        let (status, detail) = full_candidate(&layers(|_| Status::Pass), dirty);
        assert_eq!(status, Status::NotRun);
        assert!(detail.contains("uncommitted"));
        let without_session = layers(|layer| {
            if layer == Layer::ServerSessionCandidate {
                Status::NotRun
            } else {
                Status::Pass
            }
        });
        let (status, detail) = full_candidate(&without_session, committed);
        assert_eq!(status, Status::NotRun);
        assert!(detail.contains("SERVER_SESSION_CANDIDATE"));
        let kani_unsupported = layers(|layer| {
            if layer == Layer::Kani {
                Status::Unsupported
            } else {
                Status::Pass
            }
        });
        assert_eq!(
            full_candidate(&kani_unsupported, committed).0,
            Status::NotRun
        );
        let failed = layers(|layer| match layer {
            Layer::Isolation => Status::Fail,
            Layer::Kani => Status::NotRun,
            _ => Status::Pass,
        });
        let (status, detail) = full_candidate(&failed, dirty);
        assert_eq!(status, Status::Fail);
        // Only the failed layer is named as failed.
        assert_eq!(detail, "required layer failed: ISOLATION");
        let mut missing = layers(|_| Status::Pass);
        missing.remove(&Layer::Model);
        assert_eq!(full_candidate(&missing, committed).0, Status::NotRun);
    }

    #[test]
    fn the_server_owner_declares_the_agreed_session_target() {
        let workspace = Workspace::discover().unwrap();
        let manifest: toml::Table =
            std::fs::read_to_string(workspace.path("services/cantos-server/Cargo.toml"))
                .unwrap()
                .parse()
                .unwrap();
        let targets = manifest["test"].as_array().unwrap();
        let session: Vec<_> = targets
            .iter()
            .filter(|target| target["name"].as_str() == Some(SESSION_TARGET))
            .collect();
        assert_eq!(session.len(), 1);
        assert_eq!(
            session[0]["required-features"],
            toml::Value::Array(vec![toml::Value::String(SESSION_FEATURE.into())])
        );
        assert_eq!(
            session[0]["path"].as_str(),
            Some("tests/identity_auth_candidate.rs")
        );
    }

    #[test]
    fn every_selected_test_names_a_real_function_and_the_allowlist_is_finite() {
        let workspace = Workspace::discover().unwrap();
        let mut names = std::collections::BTreeSet::new();
        for selected in SELECTED {
            assert!(
                names.insert(selected.test),
                "{} selected twice",
                selected.test
            );
            let function = selected.test.rsplit("::").next().unwrap();
            let source = std::fs::read_to_string(workspace.path(selected.source)).unwrap();
            assert!(
                source.contains(&format!("async fn {function}()")),
                "{} has no test function {function}",
                selected.source
            );
            assert!(!selected.test.contains('*') && !selected.test.is_empty());
            // A child is bound only for tests that start the Identity executable.
            // A bound child only for tests in files that start the real Identity executable.
            if selected.child {
                assert!(
                    source.contains("IdentityEnv::supplied") || source.contains("env.serve()"),
                    "{}",
                    selected.test
                );
            } else {
                assert!(
                    !source.contains("IdentityEnv::supplied"),
                    "{}",
                    selected.test
                );
            }
        }
        for layer in [
            Layer::IdentityProcessRpc,
            Layer::ActorTurso,
            Layer::ServerSessionCandidate,
        ] {
            assert!(SELECTED.iter().any(|selected| selected.layer == layer));
        }
        assert!(SELECTED
            .iter()
            .any(|selected| selected.unit == Unit::Test(SESSION_TARGET)
                && selected.test == SESSION_TEST));
    }

    #[test]
    fn the_plan_builds_first_runs_only_guarded_exact_tests_and_is_opt_in() {
        let workspace = Workspace::discover().unwrap();
        let plan = plan(&workspace);
        let builds: Vec<&str> = plan["builds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b.as_str().unwrap())
            .collect();
        assert!(builds.iter().all(|build| build.contains("--locked")
            && build.contains("--message-format=json-render-diagnostics")));
        assert!(builds
            .iter()
            .any(|build| build.contains("--test identity_auth_candidate --no-run")));
        assert!(builds.iter().any(|build| build.contains("--lib --no-run")));
        assert_eq!(plan["layers"].as_array().unwrap().len(), Layer::ALL.len());
        assert_eq!(
            plan["kernels"],
            json!([
                "identity-security-version",
                "identity-completion",
                "identity-mailbox-proof"
            ])
        );
        // Never part of ordinary gates, preflight or CI jobs.
        let suite = crate::cli::VerifySuite::NativeAuthQualification(QualificationArgs {
            kani: true,
            report: None,
            dry_run: false,
        });
        assert!(crate::gate::verify_suite(&suite).is_empty());
        assert!(crate::gate::full()
            .iter()
            .flat_map(|gate| gate.commands(&workspace))
            .all(|command| !command
                .args
                .iter()
                .any(|arg| arg.contains("native-auth-qualification"))));
    }

    #[test]
    fn the_retained_identity_executable_is_an_exact_copy_that_later_builds_cannot_replace() {
        let directory = std::env::temp_dir().join(format!("cantos-retain-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let built = directory.join("built");
        std::fs::write(&built, b"first build").unwrap();
        let destination = directory.join("owned/cantos-identity");
        let (digest, path) = retain(&built, &destination).unwrap();
        assert_eq!(path, destination);
        assert_eq!(digest, sha256_file(&built).unwrap());
        // A later build replacing the Cargo output leaves the retained file and its digest.
        std::fs::write(&built, b"re-linked by a test build").unwrap();
        assert_eq!(sha256_file(&destination).unwrap(), digest);
        // Retaining again replaces the owned copy only.
        let (second, _) = retain(&built, &destination).unwrap();
        assert_ne!(second, digest);
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn the_guard_invocation_binds_the_child_only_for_tests_that_start_it() {
        let workspace = Workspace::discover().unwrap();
        for selected in SELECTED {
            let args = NativeIsolationArgs {
                test_binary: PathBuf::from("/prebuilt/test"),
                test: selected.test.to_owned(),
                timeout_seconds: TEST_TIMEOUT_SECONDS,
                identity_binary: selected
                    .child
                    .then(|| PathBuf::from("/prebuilt/cantos-identity")),
                dry_run: false,
            };
            let spec = native_isolation::command(&workspace, &args);
            assert_eq!(
                spec.args.iter().any(|arg| arg == "--identity-binary"),
                selected.child
            );
            assert!(spec.args.iter().any(|arg| arg == selected.test));
        }
    }
}
