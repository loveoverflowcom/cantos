//! Explicit synthetic Theatre data for the native development composition.
//!
//! A Turso file has one owner process, so the order is fixed and nothing runs concurrently
//! against `server.db`: build, `cantos-migrate`, `cantos-operator` (creator actor, then a
//! development token), start the Server, run the HTTP seeder with that token, stop the Server.
//! The seeder publishes through the real Studio routes of the running Server; nothing writes
//! rows around the domain gates. The token is captured from the operator's private temporary
//! output, handed to the seeder as a redacted environment value and never printed.
//!
//! This is a developer fixture, never a gate, a preflight selection or a CI job. Synthetic
//! rights/QC records and a few seconds of owned tone are not native playback or human-listening
//! evidence.
//!
//! The retired named `--environment` lane is refused before any effect (see
//! [`crate::retired`]); `--reset` removes only the files this composition created.

use std::{path::Path, process::ExitCode, time::Duration};

use anyhow::{anyhow, ensure, Context, Result};
use clap::{Args, ValueEnum};
use serde_json::Value;

use crate::{
    build::{self, Profile, Refusal},
    capture::{Capture, SystemCapture},
    command::{CommandSpec, Runner},
    config::{loopback, App, API_PACKAGE, API_PORT, SERVER_MIGRATE_BIN, SERVER_OPERATOR_BIN},
    native_dev::{self, Layout, WithIdentity},
    probe::{self, Endpoint, Readiness},
    process::Supervisor,
    report::{Report, Status},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

mod dataset;
mod sample;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SeedTarget {
    /// Public Theatre catalog: novel, story and podcast Works published through Studio flows.
    Theatre,
    /// Explicit private localhost data profiles; separate from the fixed Theatre sample.
    Dataset,
}

/// The agreed interface to the HTTP seeder: an ordinary Cargo integration target of the
/// Server whose exact, explicitly ignored test drives the real Studio HTTP routes of a running
/// Server with a bearer-style development token. It is owned by the Server's tests; when the
/// target file is absent `seed theatre` reports `UNSUPPORTED` before any effect.
pub const SEED_TARGET: &str = "catalog_http";
pub const SEED_TEST: &str =
    "audio_lifecycle_http::publication_http::theatre_dev_seed_http::theatre_development_catalog_seed";
pub const SEED_DATASET_ENVIRONMENT: &str = "local-development";
const SEED_RIGHTS_REF: &str = "synthetic-owned";
/// Development cookie name of the development host, the only one a loopback Server accepts.
const SEED_COOKIE_NAME: &str = "cantos_session";
const TOKEN_HOURS: &str = "24";
const FATES: [&str; 4] = ["public", "withdrawn", "private", "unpublished"];
const OPERATOR_TIMEOUT: Duration = Duration::from_secs(120);
const SERVER_READY_TIMEOUT: Duration = Duration::from_secs(60);

/// `--reset` is the one live option. The remaining fields only exist so the retired named
/// environment lane parses and can refuse with its reason instead of a generic usage error.
#[derive(Args, Debug, Default, Clone, PartialEq, Eq)]
pub struct SeedOptions {
    #[command(flatten)]
    pub dataset: dataset::Options,
    /// Remove this composition's server.db, seed media, receipt and creator record.
    #[arg(long)]
    pub reset: bool,
    /// Prepared sample-catalog.json from issue #427; never a live database input.
    #[arg(long, value_name = "PATH", requires_all = ["sample_root", "sample_media_source"])]
    pub sample_input: Option<std::path::PathBuf>,
    /// Directory containing the six original covers/<sample-key>.avif files.
    #[arg(long, value_name = "DIR", requires = "sample_input")]
    pub sample_media_source: Option<std::path::PathBuf>,
    /// Isolated output directory outside Git; --reset archives only this owned dataset.
    #[arg(long, value_name = "DIR")]
    pub sample_root: Option<std::path::PathBuf>,
    /// Keep the verified sample Server on 127.0.0.1:8080 until Ctrl-C.
    #[arg(long, requires = "sample_input", conflicts_with = "reset")]
    pub sample_serve: bool,
    #[arg(long, hide = true, value_name = "NAME")]
    pub environment: Option<String>,
    #[arg(long, hide = true, value_name = "ORIGIN")]
    pub api_origin: Option<String>,
    #[arg(long, hide = true, value_name = "ORIGIN")]
    pub web_origin: Option<String>,
    #[arg(long, hide = true, value_name = "VARIABLE")]
    pub database_url_env: Option<String>,
    #[arg(long, hide = true, value_name = "VARIABLE")]
    pub studio_token_env: Option<String>,
    #[arg(long, hide = true, value_name = "NAME")]
    pub studio_cookie_name: Option<String>,
    #[arg(long, hide = true, value_name = "ID")]
    pub rights_ref: Option<String>,
    #[arg(long, hide = true, value_name = "PATH")]
    pub private_root: Option<std::path::PathBuf>,
    #[arg(long, hide = true, value_name = "PATH")]
    pub publication_root: Option<std::path::PathBuf>,
    #[arg(long, hide = true, value_name = "PATH")]
    pub record: Option<std::path::PathBuf>,
}

impl SeedOptions {
    /// Any input of the retired persistent-environment lane.
    pub fn is_retired(&self) -> bool {
        self.environment.is_some()
            || self.api_origin.is_some()
            || self.web_origin.is_some()
            || self.database_url_env.is_some()
            || self.studio_token_env.is_some()
            || self.studio_cookie_name.is_some()
            || self.rights_ref.is_some()
            || self.private_root.is_some()
            || self.publication_root.is_some()
            || self.record.is_some()
    }
}

fn seeder_target_file(workspace: &Workspace) -> std::path::PathBuf {
    workspace.path(&format!("services/cantos-server/tests/{SEED_TARGET}.rs"))
}

/// What a seed run needs, resolved without touching anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedPlan {
    pub layout: Layout,
    pub profile: Profile,
    pub media_origin: String,
}

pub fn plan(
    workspace: &Workspace,
    target: SeedTarget,
    options: &SeedOptions,
) -> Result<SeedPlan, Refusal> {
    if target != SeedTarget::Theatre {
        return Err(Refusal::Usage(
            "dataset profiles require the dataset planner".into(),
        ));
    }
    if options.is_retired() {
        return Err(crate::retired::Retirement::SeedEnvironment.refusal());
    }
    let layout = Layout::for_current_user(workspace)?;
    let media_origin =
        native_dev::local_media_origin(std::env::var("CANTOS_LOCAL_MEDIA_ORIGIN").ok().as_deref())?;
    if !options.reset && !seeder_target_file(workspace).is_file() {
        return Err(Refusal::Unsupported {
            subject: "seed theatre".into(),
            reason: format!(
                "the HTTP seeder (the `{SEED_TARGET}` integration test target of {API_PACKAGE}) is not present in this tree; the former seeder ran against PostgreSQL and has not been ported to the native Server yet. No file or process was touched"
            ),
        });
    }
    Ok(SeedPlan {
        layout,
        profile: Profile::Dev,
        media_origin,
    })
}

/// The Server the seed talks to: the development composition without Identity, with the seed's
/// own media roots so uploads, promotion and delivery are real.
pub fn seed_server_command(workspace: &Workspace, plan: &SeedPlan) -> CommandSpec {
    native_dev::server_serve(
        workspace,
        &plan.layout,
        plan.profile,
        &App::Studio.web_origin(),
        WithIdentity::No,
    )
    .env(
        "CANTOS_AUDIO_ROOT",
        plan.layout.media_private().display().to_string(),
    )
    .env(
        "CANTOS_PUBLICATION_ROOT",
        plan.layout.media_publication().display().to_string(),
    )
    .env(
        "CANTOS_PLAYBACK_KEY_FILE",
        plan.layout.media_key().display().to_string(),
    )
    .env("CANTOS_PUBLIC_MEDIA_ORIGIN", plan.media_origin.clone())
}

pub fn seeder_command(workspace: &Workspace, plan: &SeedPlan, token: &str) -> CommandSpec {
    let layout = &plan.layout;
    CommandSpec::new("cargo", workspace.root())
        .args([
            "test",
            "-p",
            API_PACKAGE,
            "--test",
            SEED_TARGET,
            "--locked",
            "--",
            "--ignored",
            "--exact",
            SEED_TEST,
            "--test-threads=1",
        ])
        .env("CANTOS_THEATRE_SEED_ENVIRONMENT", SEED_DATASET_ENVIRONMENT)
        .env(
            "CANTOS_THEATRE_SEED_API_ORIGIN",
            format!("http://{}", loopback(API_PORT)),
        )
        .env("CANTOS_THEATRE_SEED_WEB_ORIGIN", App::Studio.web_origin())
        .env_secret("CANTOS_THEATRE_SEED_STUDIO_TOKEN", token)
        .env("CANTOS_THEATRE_SEED_STUDIO_COOKIE_NAME", SEED_COOKIE_NAME)
        .env("CANTOS_THEATRE_SEED_RIGHTS_REF", SEED_RIGHTS_REF)
        .env(
            "CANTOS_THEATRE_SEED_PRIVATE_ROOT",
            layout.media_private().display().to_string(),
        )
        .env(
            "CANTOS_THEATRE_SEED_PUBLICATION_ROOT",
            layout.media_publication().display().to_string(),
        )
        .env(
            "CANTOS_THEATRE_SEED_PLAYBACK_KEY_FILE",
            layout.media_key().display().to_string(),
        )
        .env(
            "CANTOS_THEATRE_SEED_RECORD",
            layout.seed_receipt.display().to_string(),
        )
        .env(
            "CANTOS_THEATRE_SEED_JOURNAL",
            layout.seed_journal.display().to_string(),
        )
}

/// A creator actor id or a development token as printed by the operator: one short token of
/// printable ASCII. Anything else is refused rather than interpolated anywhere.
fn single_token(output: &str, what: &str) -> Result<String> {
    let value = output.trim();
    ensure!(
        !value.is_empty()
            && value.len() <= 512
            && value.bytes().all(|byte| byte.is_ascii_graphic()),
        "the operator printed no usable {what}"
    );
    Ok(value.to_owned())
}

/// Starts and stops the one Server the seed needs. A trait so the order is testable.
pub trait ServerHost {
    fn start(&mut self, name: &str, command: &CommandSpec, endpoint: &Endpoint) -> Result<()>;
    fn stop(&mut self);
}

struct SupervisedServer {
    supervisor: Option<Supervisor>,
    root: std::path::PathBuf,
}

impl ServerHost for SupervisedServer {
    fn start(&mut self, name: &str, command: &CommandSpec, endpoint: &Endpoint) -> Result<()> {
        ensure!(
            matches!(endpoint, Endpoint::Tcp(addr) if probe::port_is_free(*addr)),
            "{endpoint} is already in use; xtask does not stop processes it did not start"
        );
        let mut supervisor = Supervisor::new()?;
        println!("\n==> start {name}: {}", command.display(&self.root));
        supervisor.spawn(name, command)?;
        let ready = supervisor
            .await_endpoint(endpoint, &Readiness::Tcp, SERVER_READY_TIMEOUT)
            .context("the seed Server did not become ready")?;
        self.supervisor = Some(supervisor);
        ensure!(ready, "interrupted before the Server became ready");
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut supervisor) = self.supervisor.take() {
            supervisor.shutdown();
        }
    }
}

/// Records each stage; after the first failure the rest are `NOT_RUN`, never skipped silently.
struct Stages {
    report: Report,
    failed: bool,
}

impl Stages {
    fn run<T>(&mut self, name: &str, stage: impl FnOnce() -> Result<(T, String)>) -> Option<T> {
        if self.failed {
            self.report
                .record(name, Status::NotRun, "not started: an earlier stage failed");
            return None;
        }
        println!("\n==> {name}");
        match stage() {
            Ok((value, detail)) => {
                self.report.record(name, Status::Pass, detail);
                Some(value)
            }
            Err(error) => {
                self.failed = true;
                self.report.record(name, Status::Fail, format!("{error:#}"));
                None
            }
        }
    }
}

fn read_actor(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => single_token(&text, "creator actor id").map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("create {}", path.display()))?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(_: &Path, _: &[u8]) -> Result<()> {
    bail!("the native development composition needs a Unix host")
}

/// Private media roots and a 32-byte playback key, created once and reused by later seeds.
#[cfg(unix)]
fn ensure_media(layout: &Layout) -> Result<String> {
    use std::{io::Read, os::unix::fs::DirBuilderExt};
    let mut created = Vec::new();
    for directory in [
        layout.media_dir.clone(),
        layout.media_private(),
        layout.media_publication(),
    ] {
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => created.push(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).with_context(|| format!("create {}", directory.display()))
            }
        }
    }
    if !layout.media_key().exists() {
        let mut key = [0u8; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut key))
            .context("read the playback key from the operating system")?;
        write_private(&layout.media_key(), &key)?;
        created.push(layout.media_key());
    }
    Ok(format!("{} new private path(s)", created.len()))
}

#[cfg(not(unix))]
fn ensure_media(_: &Layout) -> Result<String> {
    bail!("the native development composition needs a Unix host")
}

fn operator_failure(error: impl std::fmt::Display, stderr: &str) -> anyhow::Error {
    // stdout is never included: it may hold a credential.
    let detail: String = stderr.trim().chars().take(400).collect();
    anyhow!(
        "cantos-operator failed: {error}{}",
        if detail.is_empty() {
            String::new()
        } else {
            format!(" ({detail})")
        }
    )
}

fn capture_operator(capture: &mut dyn Capture, spec: &CommandSpec, what: &str) -> Result<String> {
    let captured = capture
        .capture(spec, OPERATOR_TIMEOUT)
        .map_err(|error| anyhow!("cantos-operator {what}: {error}"))?;
    ensure!(
        captured.code == 0,
        "{}",
        operator_failure(format!("{what} exited {}", captured.code), &captured.stderr)
    );
    single_token(&captured.stdout, what)
}

/// Checks the receipt the seeder wrote before it is reported or served.
pub fn validate_receipt(receipt: &Value, layout: &Layout) -> Result<String> {
    ensure!(
        receipt.get("format").and_then(Value::as_i64) == Some(1)
            || receipt.get("dataset").and_then(Value::as_str).is_some(),
        "seed receipt has no recognized format"
    );
    let works = receipt
        .get("works")
        .and_then(Value::as_array)
        .filter(|works| !works.is_empty())
        .ok_or_else(|| anyhow!("seed receipt recorded no works"))?;
    let mut lines = vec![format!(
        "Theatre seed catalog ({} works) in {}:",
        works.len(),
        layout.server_db.display()
    )];
    for work in works {
        let fate = work.get("fate").and_then(Value::as_str).unwrap_or_default();
        ensure!(
            FATES.contains(&fate),
            "seed receipt has an invalid work fate"
        );
        let units = work
            .get("units")
            .and_then(Value::as_array)
            .filter(|units| !units.is_empty())
            .ok_or_else(|| anyhow!("seed receipt recorded a work without units"))?;
        let visible = if fate == "public" {
            units
                .iter()
                .filter(|unit| {
                    unit.get("publication_id").is_some_and(|id| !id.is_null())
                        && unit.get("withdrawn") != Some(&Value::Bool(true))
                })
                .count()
        } else {
            0
        };
        lines.push(format!(
            "  {fate:<11} {:<7} {}  {visible}/{} public units  {}",
            work.get("kind").and_then(Value::as_str).unwrap_or("?"),
            work.get("work_id").and_then(Value::as_str).unwrap_or("?"),
            units.len(),
            work.get("title").and_then(Value::as_str).unwrap_or("")
        ));
    }
    lines.push("Manifest/grant/byte acceptance is separate from native playback.".into());
    Ok(lines.join("\n"))
}

/// The whole seed flow, parameterized over its effects.
pub fn run_seed(
    workspace: &Workspace,
    plan: &SeedPlan,
    runner: &mut dyn Runner,
    capture: &mut dyn Capture,
    host: &mut dyn ServerHost,
) -> Report {
    let layout = &plan.layout;
    let mut stages = Stages {
        report: Report::new("seed theatre"),
        failed: false,
    };
    stages.run("private development directories", || {
        native_dev::ensure_private_directories(layout)?;
        Ok(((), workspace.relative(&layout.root)))
    });
    stages.run("private media roots and playback key", || {
        Ok(((), ensure_media(layout)?))
    });
    stages.run("build Server binaries", || {
        for bin in [
            crate::config::API_SERVER_BIN,
            SERVER_MIGRATE_BIN,
            SERVER_OPERATOR_BIN,
        ] {
            runner.run(&build::api_binaries(workspace, plan.profile, Some(bin)))?;
        }
        Ok(((), String::new()))
    });
    stages.run("migrate server.db (Server stopped)", || {
        runner.run(&native_dev::migrate_server(workspace, layout, plan.profile))?;
        Ok(((), String::new()))
    });
    let actor = stages.run("creator actor (cantos-operator)", || {
        if let Some(actor) = read_actor(&layout.seed_actor)? {
            return Ok((actor, "reused the recorded creator actor".to_owned()));
        }
        let spec = native_dev::operator(workspace, layout, plan.profile, &["create-creator"]);
        let actor = capture_operator(capture, &spec, "create-creator")?;
        write_private(&layout.seed_actor, actor.as_bytes())?;
        Ok((actor, "created a creator actor".to_owned()))
    });
    let token = stages.run("development token (cantos-operator, never printed)", || {
        let actor = actor
            .as_deref()
            .ok_or_else(|| anyhow!("no creator actor"))?;
        let spec = native_dev::operator(
            workspace,
            layout,
            plan.profile,
            &["issue-token", actor, TOKEN_HOURS],
        );
        let token = capture_operator(capture, &spec, "issue-token")?;
        Ok((token, format!("valid {TOKEN_HOURS} hours")))
    });
    let started = stages
        .run("start Server (sole owner of server.db)", || {
            host.start(
                crate::config::API_SERVER_BIN,
                &seed_server_command(workspace, plan),
                &Endpoint::Tcp(loopback(API_PORT)),
            )?;
            Ok(((), String::new()))
        })
        .is_some();
    stages.run("seed the catalog over HTTP", || {
        let token = token.as_deref().ok_or_else(|| anyhow!("no token"))?;
        ensure!(started, "the Server is not running");
        runner.run(&seeder_command(workspace, plan, token))?;
        Ok(((), String::new()))
    });
    // The Server must release server.db whatever happened above.
    host.stop();
    stages.run("receipt", || {
        let text = std::fs::read_to_string(&layout.seed_receipt)
            .map_err(|_| anyhow!("the seeder wrote no receipt; no seed is recorded"))?;
        let receipt: Value =
            serde_json::from_str(&text).map_err(|_| anyhow!("the seed receipt is not JSON"))?;
        let summary = validate_receipt(&receipt, layout)?;
        println!("{summary}");
        Ok(((), "valid".to_owned()))
    });
    stages
        .report
        .note("Synthetic rights/QC records and owned tones are not native playback or human-listening evidence");
    stages.report
}

/// Removes only what this composition created for the seed. Refuses while anything serves the
/// API port (it could own `server.db`). Identity's file is never touched.
pub fn reset(plan: &SeedPlan, api_port_free: bool) -> Result<Report, Refusal> {
    if !api_port_free {
        return Err(Refusal::Usage(format!(
            "something is listening on 127.0.0.1:{API_PORT}; stop that Server before `seed theatre --reset` (xtask never stops a process it did not start)"
        )));
    }
    let layout = &plan.layout;
    let mut report = Report::new("seed theatre --reset");
    let mut removed = Vec::new();
    for directory in [&layout.server_dir, &layout.media_dir] {
        match std::fs::symlink_metadata(directory) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                std::fs::remove_dir_all(directory).map_err(|error| {
                    Refusal::Usage(format!("remove {}: {error}", directory.display()))
                })?;
                removed.push(directory.display().to_string());
            }
            Ok(_) => {
                return Err(Refusal::Usage(format!(
                    "{} is not a directory xtask created; remove it yourself",
                    directory.display()
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(Refusal::Usage(format!(
                    "inspect {}: {error}",
                    directory.display()
                )))
            }
        }
    }
    for file in [
        &layout.seed_actor,
        &layout.seed_receipt,
        &layout.seed_journal,
    ] {
        if std::fs::remove_file(file).is_ok() {
            removed.push(file.display().to_string());
        }
    }
    report.record(
        "reset",
        Status::Pass,
        if removed.is_empty() {
            "nothing to reset".to_owned()
        } else {
            format!(
                "removed the Server development file, seed media and records ({} item(s)); identity.db was not touched",
                removed.len()
            )
        },
    );
    Ok(report)
}

pub fn execute(
    workspace: &Workspace,
    target: SeedTarget,
    options: &SeedOptions,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<std::result::Result<ExitCode, Refusal>> {
    if target == SeedTarget::Dataset {
        return dataset::execute(workspace, options, tools, runner);
    }
    if options.dataset != dataset::Options::default() {
        return Ok(Err(Refusal::Usage(
            "dataset flags require `seed dataset`".into(),
        )));
    }
    if options.sample_root.is_some()
        || options.sample_input.is_some()
        || options.sample_media_source.is_some()
        || options.sample_serve
    {
        return sample::execute(workspace, options, tools, runner);
    }
    let plan = match plan(workspace, target, options) {
        Ok(plan) => plan,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if options.reset {
        return Ok(
            reset(&plan, probe::port_is_free(loopback(API_PORT))).map(|report| {
                report.print();
                report.exit_code()
            }),
        );
    }
    let missing = tools.missing(&[Tool::Cargo, Tool::Ffmpeg, Tool::Ffprobe]);
    if !missing.is_empty() {
        let names: Vec<_> = missing.iter().map(|tool| tool.label()).collect();
        return Ok(Err(Refusal::Unsupported {
            subject: "seed theatre".into(),
            reason: format!(
                "missing {}; no file or process was touched",
                names.join(", ")
            ),
        }));
    }
    if !probe::port_is_free(loopback(API_PORT)) {
        return Ok(Err(Refusal::Usage(format!(
            "127.0.0.1:{API_PORT} is already in use; the seed needs the Server stopped because server.db has one owner, and xtask never stops a process it did not start"
        ))));
    }
    let mut capture = SystemCapture::new()?;
    let mut host = SupervisedServer {
        supervisor: None,
        root: workspace.root().to_path_buf(),
    };
    let report = run_seed(workspace, &plan, runner, &mut capture, &mut host);
    report.print();
    Ok(Ok(report.exit_code()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        capture::{CaptureError, Captured},
        command::testing::RecordingRunner,
        tools::testing::FakeTools,
    };
    use std::cell::RefCell;

    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A throwaway workspace that holds a seeder target file, so planning succeeds.
    fn fixture(with_seeder: bool) -> (Root, Workspace) {
        // Short, so the Identity socket path stays within `sun_path` whatever TMPDIR is.
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::path::PathBuf::from("/tmp").join(format!(
            "xs-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("services/cantos-server/tests")).unwrap();
        std::fs::create_dir_all(root.join("t")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        if with_seeder {
            std::fs::write(
                root.join(format!("services/cantos-server/tests/{SEED_TARGET}.rs")),
                "",
            )
            .unwrap();
        }
        let workspace = Workspace::fixture(&root).unwrap();
        (Root(root), workspace)
    }

    fn plan_in(workspace: &Workspace) -> SeedPlan {
        // SAFETY: geteuid has no preconditions and cannot fail.
        let uid = unsafe { libc::geteuid() };
        let layout = Layout::resolve(workspace, uid, &workspace.path("t")).unwrap();
        SeedPlan {
            layout,
            profile: Profile::Dev,
            media_origin: native_dev::DEFAULT_MEDIA_ORIGIN.into(),
        }
    }

    #[derive(Default)]
    struct FakeCapture {
        calls: Vec<String>,
        fail_token: bool,
    }
    impl Capture for FakeCapture {
        fn capture(&mut self, spec: &CommandSpec, _: Duration) -> Result<Captured, CaptureError> {
            let verb = spec.args.first().cloned().unwrap_or_default();
            self.calls.push(spec.args.join(" "));
            if verb == "issue-token" && self.fail_token {
                return Ok(Captured {
                    code: 1,
                    stdout: "must-not-leak-token\n".into(),
                    stderr: "operator command failed".into(),
                });
            }
            Ok(Captured {
                code: 0,
                stdout: if verb == "create-creator" {
                    "actor-0001\n".into()
                } else {
                    "dev-token-secret-value\n".into()
                },
                stderr: String::new(),
            })
        }
    }

    #[derive(Default)]
    struct FakeHost {
        events: RefCell<Vec<String>>,
    }
    impl ServerHost for FakeHost {
        fn start(&mut self, name: &str, command: &CommandSpec, _: &Endpoint) -> Result<()> {
            self.events
                .borrow_mut()
                .push(format!("start {name} identity={}", {
                    command
                        .env
                        .iter()
                        .any(|(key, _)| key == "CANTOS_IDENTITY_SOCKET")
                }));
            Ok(())
        }
        fn stop(&mut self) {
            self.events.borrow_mut().push("stop".into());
        }
    }

    /// A runner that also records its ordering relative to the Server's start and stop.
    struct OrderedRunner<'a> {
        inner: RecordingRunner,
        host: &'a FakeHost,
        /// The seeder writes the receipt; the fake does so when the seeder command runs.
        receipt_for: Option<Layout>,
    }
    impl Runner for OrderedRunner<'_> {
        fn run(&mut self, spec: &CommandSpec) -> Result<()> {
            let label = std::path::Path::new(&spec.program)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let event = if label == "cargo" && spec.args.first().is_some_and(|arg| arg == "test") {
                "run seeder".to_owned()
            } else {
                format!("run {label}")
            };
            self.host.events.borrow_mut().push(event);
            if let Some(layout) = &self.receipt_for {
                if label == "cargo" && spec.args.contains(&"--ignored".to_owned()) {
                    write_receipt(layout);
                }
            }
            self.inner.run(spec)
        }
    }

    fn write_receipt(layout: &Layout) {
        std::fs::create_dir_all(&layout.root).unwrap();
        std::fs::write(
            &layout.seed_receipt,
            serde_json::json!({
                "format": 1,
                "works": [
                    {"fate": "public", "kind": "novel", "work_id": "w1", "title": "Mùa gió chướng",
                     "units": [{"publication_id": "p1", "withdrawn": false}]},
                    {"fate": "private", "kind": "story", "work_id": "w2", "title": "Riêng tư",
                     "units": [{"publication_id": null}]}
                ]
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn the_seed_never_runs_two_owners_of_server_db_and_orders_migrate_operator_server_seeder() {
        let (_root, workspace) = fixture(true);
        let plan = plan_in(&workspace);
        let host = FakeHost::default();
        let mut runner = OrderedRunner {
            inner: RecordingRunner::default(),
            host: &host,
            receipt_for: Some(plan.layout.clone()),
        };
        let mut capture = FakeCapture::default();
        // One event log is shared between the runner and the host so ordering is observable.
        let report = {
            let mut shared = SharedHost { host: &host };
            run_seed(&workspace, &plan, &mut runner, &mut capture, &mut shared)
        };
        let events = host.events.borrow().clone();
        let position = |needle: &str| {
            events
                .iter()
                .position(|event| event.contains(needle))
                .unwrap_or_else(|| panic!("{needle} missing in {events:?}"))
        };
        assert!(position("run cantos-migrate") < position("start cantos-server"));
        assert!(position("start cantos-server") < position("run seeder"));
        assert!(position("run seeder") < position("stop"));
        // Operator commands are captured (they own the file), and only while the Server is down.
        assert_eq!(
            capture.calls,
            ["create-creator", "issue-token actor-0001 24"]
        );
        // The seed Server composes no Identity: its development token needs none.
        assert!(events.contains(&"start cantos-server identity=false".to_owned()));
        assert!(report.succeeded(), "{:?}", report.outcomes);
        // The Server was stopped exactly once, after the seeder.
        assert_eq!(events.iter().filter(|event| *event == "stop").count(), 1);
    }

    struct SharedHost<'a> {
        host: &'a FakeHost,
    }
    impl ServerHost for SharedHost<'_> {
        fn start(&mut self, name: &str, command: &CommandSpec, endpoint: &Endpoint) -> Result<()> {
            let mut host = FakeHost::default();
            host.start(name, command, endpoint)?;
            self.host.events.borrow_mut().extend(host.events.take());
            Ok(())
        }
        fn stop(&mut self) {
            self.host.events.borrow_mut().push("stop".into());
        }
    }

    #[test]
    fn the_token_reaches_only_the_seeder_environment_and_is_never_rendered() {
        let (_root, workspace) = fixture(true);
        let plan = plan_in(&workspace);
        let seeder = seeder_command(&workspace, &plan, "dev-token-secret-value");
        assert!(seeder
            .env
            .iter()
            .any(|(key, value)| key == "CANTOS_THEATRE_SEED_STUDIO_TOKEN"
                && value == "dev-token-secret-value"));
        let shown = seeder.display(workspace.root());
        assert!(!shown.contains("dev-token-secret-value"), "{shown}");
        assert!(shown.contains("CANTOS_THEATRE_SEED_STUDIO_TOKEN=<redacted>"));
        // No argument carries it, and no database URL variable exists.
        assert!(seeder.args.iter().all(|arg| !arg.contains("dev-token")));
        assert!(seeder
            .env
            .iter()
            .all(|(key, _)| !key.contains("DATABASE_URL") && !key.contains("CLUSTER")));
        assert_eq!(
            &seeder.args[..6],
            [
                "test",
                "-p",
                "cantos-server",
                "--test",
                "catalog_http",
                "--locked"
            ]
        );
        assert!(seeder.args.contains(&"--ignored".to_owned()));
        assert!(seeder.args.contains(&"--exact".to_owned()));
    }

    #[test]
    fn a_failed_operator_stops_before_the_server_starts_and_never_leaks_stdout() {
        let (_root, workspace) = fixture(true);
        let plan = plan_in(&workspace);
        let host = FakeHost::default();
        let mut runner = OrderedRunner {
            inner: RecordingRunner::default(),
            host: &host,
            receipt_for: None,
        };
        let mut capture = FakeCapture {
            fail_token: true,
            ..Default::default()
        };
        let mut shared = SharedHost { host: &host };
        let report = run_seed(&workspace, &plan, &mut runner, &mut capture, &mut shared);
        assert!(!report.succeeded());
        let events = host.events.borrow().clone();
        assert!(!events.iter().any(|event| event.starts_with("start")));
        assert!(!events.iter().any(|event| event.contains("run seeder")));
        // The Server is stopped even though it never started: stop is idempotent.
        assert!(events.contains(&"stop".to_owned()));
        let text = format!("{:?}", report.outcomes);
        assert!(!text.contains("must-not-leak-token"), "{text}");
        let statuses: Vec<_> = report
            .outcomes
            .iter()
            .map(|outcome| outcome.status)
            .collect();
        assert!(statuses.contains(&Status::Fail));
        // Stages after the failure are NOT_RUN, never PASS.
        assert!(report
            .outcomes
            .iter()
            .filter(|outcome| outcome.name.starts_with("seed the catalog"))
            .all(|outcome| outcome.status == Status::NotRun));
    }

    #[test]
    fn a_missing_seeder_target_is_unsupported_before_any_file_or_process() {
        let (root, workspace) = fixture(false);
        let outcome = plan(&workspace, SeedTarget::Theatre, &SeedOptions::default());
        match outcome {
            Err(Refusal::Unsupported { reason, .. }) => {
                assert!(reason.contains("catalog_http"));
                assert!(reason.contains("No file or process was touched"));
            }
            other => panic!("expected UNSUPPORTED, got {other:?}"),
        }
        assert!(!workspace.path(native_dev::DEV_ROOT).exists());
        drop(root);
    }

    #[test]
    fn the_named_persistent_environment_lane_refuses_before_any_effect() {
        let (_root, workspace) = fixture(true);
        let mut runner = RecordingRunner::default();
        for options in [
            SeedOptions {
                environment: Some("catalog-local".into()),
                ..Default::default()
            },
            SeedOptions {
                database_url_env: Some("SEED_DB".into()),
                ..Default::default()
            },
            SeedOptions {
                studio_cookie_name: Some("__Secure-cantos_studio".into()),
                ..Default::default()
            },
        ] {
            assert!(options.is_retired());
            let outcome = execute(
                &workspace,
                SeedTarget::Theatre,
                &options,
                &FakeTools::all(),
                &mut runner,
            )
            .unwrap();
            let Err(Refusal::Usage(message)) = outcome else {
                panic!("expected a usage refusal");
            };
            assert!(
                message.contains("retired") && message.contains("one owner"),
                "{message}"
            );
        }
        assert!(runner.commands.is_empty());
        assert!(!workspace.path(native_dev::DEV_ROOT).exists());
        assert!(!SeedOptions::default().is_retired());
        assert!(!SeedOptions {
            reset: true,
            ..Default::default()
        }
        .is_retired());
    }

    #[test]
    fn missing_media_tools_are_unsupported_and_touch_nothing() {
        let (_root, workspace) = fixture(true);
        let mut runner = RecordingRunner::default();
        for missing in [Tool::Ffmpeg, Tool::Ffprobe, Tool::Cargo] {
            let outcome = execute(
                &workspace,
                SeedTarget::Theatre,
                &SeedOptions::default(),
                &FakeTools::without(&[missing]),
                &mut runner,
            )
            .unwrap();
            let Err(Refusal::Unsupported { reason, .. }) = outcome else {
                panic!("expected UNSUPPORTED");
            };
            assert!(reason.contains(missing.label()));
        }
        assert!(runner.commands.is_empty());
        assert!(!workspace.path(native_dev::DEV_ROOT).exists());
    }

    #[test]
    fn reset_refuses_while_the_api_port_is_taken_and_removes_only_its_own_files() {
        let (_root, workspace) = fixture(false);
        let plan = plan_in(&workspace);
        std::fs::create_dir_all(&plan.layout.server_dir).unwrap();
        std::fs::write(&plan.layout.server_db, "db").unwrap();
        std::fs::create_dir_all(&plan.layout.identity_dir).unwrap();
        std::fs::write(&plan.layout.identity_db, "identity").unwrap();
        std::fs::create_dir_all(plan.layout.media_private()).unwrap();
        std::fs::write(&plan.layout.seed_actor, "actor").unwrap();
        std::fs::write(&plan.layout.seed_receipt, "{}").unwrap();
        assert!(matches!(reset(&plan, false), Err(Refusal::Usage(_))));
        assert!(plan.layout.server_db.exists(), "refusal must not delete");
        let report = reset(&plan, true).unwrap();
        assert_eq!(report.outcomes[0].status, Status::Pass);
        assert!(!plan.layout.server_dir.exists() && !plan.layout.media_dir.exists());
        assert!(!plan.layout.seed_actor.exists() && !plan.layout.seed_receipt.exists());
        // Identity's file is a different owner's and survives a Server reset.
        assert!(plan.layout.identity_db.exists());
        let again = reset(&plan, true).unwrap();
        assert!(again.outcomes[0].detail.contains("nothing to reset"));
    }

    #[test]
    fn a_receipt_needs_works_units_and_known_fates() {
        let (_root, workspace) = fixture(false);
        let layout = plan_in(&workspace).layout;
        assert!(validate_receipt(&serde_json::json!({"format": 1, "works": []}), &layout).is_err());
        assert!(validate_receipt(
            &serde_json::json!({"format": 1, "works": [{"fate": "odd", "units": [{}]}]}),
            &layout
        )
        .is_err());
        assert!(validate_receipt(
            &serde_json::json!({"format": 1, "works": [{"fate": "public", "units": []}]}),
            &layout
        )
        .is_err());
        assert!(validate_receipt(&serde_json::json!({"works": [1]}), &layout).is_err());
        write_receipt(&layout);
        let receipt: Value =
            serde_json::from_str(&std::fs::read_to_string(&layout.seed_receipt).unwrap()).unwrap();
        let summary = validate_receipt(&receipt, &layout).unwrap();
        assert!(summary.contains("1/1 public units") && summary.contains("Mùa gió chướng"));
        assert!(summary.contains("separate from native playback"));
    }

    #[test]
    fn the_operator_must_print_one_short_printable_token() {
        assert_eq!(single_token(" abc-1 \n", "x").unwrap(), "abc-1");
        for bad in ["", "   ", "two words", "tab\there", &"x".repeat(513), "ünï"] {
            assert!(single_token(bad, "x").is_err(), "{bad:?}");
        }
    }
}
