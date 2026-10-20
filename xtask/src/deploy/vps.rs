//! Explicit deployment of the Cantos API to the production VPS over SSH ([ADR 0039]).
//!
//! A remote write, like `deploy firebase`: never reached from `build`, `run`, `verify`,
//! `preflight` or `ci`. The release image is built **here** (developer machine or CI), from a
//! `git archive` of exactly `HEAD`, so untracked/private files cannot enter it and the small VPS
//! compiles nothing. The host side is `infra/vps/bin/cantos-vps`, uploaded in the same bundle:
//! it owns backup, migration, grants, the image switch, health gating and automatic rollback.
//!
//! [`plan`] is pure (validated request + revision + run directory → ordered steps); [`execute`]
//! runs the steps through the injected [`Runner`] and stops at the first failure. `--dry-run`
//! prints the same steps and starts nothing. SSH uses the operator's own agent/config with
//! `BatchMode=yes`; xtask never reads, stores or prints a key, password or token.
//!
//! [ADR 0039]: ../../../docs/decisions/0039-vps-production-runtime.md
use std::{
    path::Path,
    process::{Command, ExitCode},
};

use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};

use crate::{
    build::Refusal,
    command::{CommandSpec, Runner},
    report::{Report, Status},
    tools::find_on_path,
    workspace::Workspace,
};

const VPS_HELP: &str = "\
Operations (exactly one; deploy is the default):
  (none)        build HEAD's API image here, upload it with the host bundle, then on the VPS:
                backup -> migrate -> grants -> switch image -> health gate (auto-rollback)
  --bootstrap   upload the host bundle and run the idempotent host setup (no image)
  --reload-browser upload the host bundle; validate/restart existing edge/API, no SQL or secrets
  --rollback    switch the API back to the previous image on the VPS
  --status      print the VPS service status
  --upgrade-epoch2-prefix  preserve a full epoch-2 [1] database; migrate a sibling to [1,2],
                verify off-host recovery, then switch the database/image pair
  --cutover-epoch2  explicitly convert the inventoried empty-business epoch-1 installation;
                rehearse, freeze, verify an off-host recovery copy, then switch DB/image together

Examples:
  cargo xtask deploy vps cantos-api --host root@203.0.113.10 --dry-run
  cargo xtask deploy vps cantos-api --host root@203.0.113.10 --bootstrap
  cargo xtask deploy vps cantos-api --host root@203.0.113.10 --smoke-origin https://api.example.org
  cargo xtask deploy vps cantos-api --host root@203.0.113.10 --rollback

SSH authenticates with your own agent or ~/.ssh configuration (BatchMode, no prompts).
Host secrets are generated and kept on the VPS. See docs/deployment/vps-production.md.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum VpsTarget {
    /// The Cantos API (both Web-origin processes share one image).
    CantosApi,
}

#[derive(Debug, Args)]
#[command(after_long_help = VPS_HELP)]
pub struct VpsArgs {
    #[command(flatten)]
    pub artifact: crate::release::ArtifactArgs,
    /// What to deploy; only the API runs on the VPS (Web apps go to Firebase Hosting).
    #[arg(value_enum)]
    pub target: VpsTarget,
    /// SSH destination, `user@host` (required; never defaulted).
    #[arg(long, value_name = "USER@HOST")]
    pub host: String,
    /// Upload the host bundle and run the idempotent host setup only.
    #[arg(long, conflicts_with_all = ["rollback", "status", "cutover_epoch2", "reload_browser"])]
    pub bootstrap: bool,
    /// Apply installed browser configuration without SQL, secret or image changes.
    #[arg(long, conflicts_with_all = ["bootstrap", "rollback", "status", "cutover_epoch2"])]
    pub reload_browser: bool,
    /// Switch the API back to the previously deployed image.
    #[arg(long, conflicts_with_all = ["bootstrap", "status", "cutover_epoch2", "reload_browser"])]
    pub rollback: bool,
    /// Print the VPS service status (read-only).
    #[arg(long, conflicts_with_all = ["bootstrap", "rollback", "cutover_epoch2", "reload_browser"])]
    pub status: bool,
    /// Explicit two-phase, lossless epoch-1 (28 migrations) to epoch-2 cutover.
    #[arg(long, conflicts_with_all = ["bootstrap", "rollback", "status", "reload_browser"])]
    pub cutover_epoch2: bool,
    /// Explicit paired sibling-database epoch-2 [1] to [1,2] upgrade (Linux x86_64 only).
    #[arg(long, conflicts_with_all = ["bootstrap", "rollback", "status", "reload_browser", "cutover_epoch2"])]
    pub upgrade_epoch2_prefix: bool,
    /// Public API origin probed from this machine afterwards, e.g. https://api.example.org.
    #[arg(long, value_name = "ORIGIN")]
    pub smoke_origin: Option<String>,
    /// Print the plan only: no build, upload or remote command.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Deploy,
    Bootstrap,
    ReloadBrowser,
    Rollback,
    Status,
    CutoverEpoch2,
    UpgradeEpoch2Prefix,
}

impl Operation {
    fn is_cutover(self) -> bool {
        matches!(self, Self::CutoverEpoch2 | Self::UpgradeEpoch2Prefix)
    }

    fn cutover_command(self) -> &'static str {
        if self == Self::UpgradeEpoch2Prefix {
            "prefix"
        } else {
            "cutover"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub host: String,
    pub operation: Operation,
    pub smoke_origin: Option<String>,
    pub dry_run: bool,
}

/// One planned command and whether it changes the remote host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: &'static str,
    pub command: CommandSpec,
    pub remote_write: bool,
}

pub const REMOTE_ROOT: &str = "/root/cantos-services";
pub const HOST_SCRIPT: &str = "/usr/local/sbin/cantos-vps";
const IMAGE_REPO: &str = "localhost/cantos-api";
// Standard Unix fsync, including parent directory entries created on a first checkout.
// A failed file/directory sync is a failed stage, never an accepted recovery copy.
const DURABLE_LOCAL_COPY: &str = r#"import os,sys
from pathlib import Path
mode, boundary, run = sys.argv[1], Path(sys.argv[2]).resolve(strict=True), Path(sys.argv[3]).resolve(strict=True)
if run == boundary or not run.is_relative_to(boundary):
    raise ValueError('cutover run must be inside its workspace')
files = {'attempt': ('cutover-attempt',), 'offhost': ('cutover-backup.tar', 'cutover-backup.tar.sha256')}[mode]
for name in files:
    with (run / name).open('rb') as stream:
        os.fsync(stream.fileno())
def sync_directory(path):
    descriptor = os.open(path, os.O_RDONLY | getattr(os, 'O_DIRECTORY', 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
if mode == 'attempt':
    sync_directory(run / 'context')
directory = run
while True:
    sync_directory(directory)
    if directory == boundary:
        break
    directory = directory.parent
"#;

/// A nonsecret per-invocation identity, independent of the deployed source revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutoverAttempt([u8; 16]);

impl CutoverAttempt {
    const PREVIEW: Self = Self([0; 16]);

    fn generate() -> Result<Self> {
        #[cfg(unix)]
        {
            use std::io::Read;
            let mut bytes = [0; 16];
            std::fs::File::open("/dev/urandom")
                .context("could not open the cutover identity source")?
                .read_exact(&mut bytes)
                .context("could not read a complete cutover identity")?;
            Ok(Self(bytes))
        }
        #[cfg(not(unix))]
        bail!("epoch2 cutover identity generation is supported only on Unix")
    }
}

impl std::fmt::Display for CutoverAttempt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn incoming_directory(request: &Request, revision: &str, attempt: &CutoverAttempt) -> String {
    if request.operation.is_cutover() {
        format!("{REMOTE_ROOT}/incoming/{revision}/{attempt}")
    } else {
        format!("{REMOTE_ROOT}/incoming/{revision}")
    }
}

fn durability_step(workspace: &Workspace, run: &Path, mode: &str) -> Step {
    Step {
        name: if mode == "attempt" {
            "persist cutover attempt"
        } else {
            "persist off-host recovery copy"
        },
        command: CommandSpec::new("python3", workspace.root()).args([
            "-c".to_owned(),
            DURABLE_LOCAL_COPY.to_owned(),
            mode.to_owned(),
            workspace.root().display().to_string(),
            run.display().to_string(),
        ]),
        remote_write: false,
    }
}

pub fn resolve(args: &VpsArgs) -> Result<Request, Refusal> {
    let VpsTarget::CantosApi = args.target;
    if !valid_ssh_destination(&args.host) {
        return Err(Refusal::Usage(
            "--host must be user@host (letters, digits, '.', '-', '_'; no options or ports)".into(),
        ));
    }
    if let Some(origin) = &args.smoke_origin {
        if !valid_https_origin(origin) {
            return Err(Refusal::Usage(
                "--smoke-origin must be an exact https origin such as https://api.example.org"
                    .into(),
            ));
        }
    }
    let operation = if args.upgrade_epoch2_prefix {
        Operation::UpgradeEpoch2Prefix
    } else {
        match (
            args.bootstrap,
            args.rollback,
            args.status,
            args.cutover_epoch2,
            args.reload_browser,
        ) {
            (true, _, _, _, _) => Operation::Bootstrap,
            (_, true, _, _, _) => Operation::Rollback,
            (_, _, true, _, _) => Operation::Status,
            (_, _, _, true, _) => Operation::CutoverEpoch2,
            (_, _, _, _, true) => Operation::ReloadBrowser,
            _ => Operation::Deploy,
        }
    };
    if operation == Operation::Status && args.smoke_origin.is_some() {
        return Err(Refusal::Usage(
            "--status is read-only and takes no --smoke-origin".into(),
        ));
    }
    Ok(Request {
        host: args.host.clone(),
        operation,
        smoke_origin: args.smoke_origin.clone(),
        dry_run: args.dry_run,
    })
}

fn valid_ssh_destination(value: &str) -> bool {
    let Some((user, host)) = value.split_once('@') else {
        return false;
    };
    let user_ok = !user.is_empty()
        && user.len() <= 32
        && user
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        && !user.starts_with('-');
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && !host.starts_with(['-', '.'])
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    user_ok && host_ok
}

fn valid_https_origin(value: &str) -> bool {
    let Some(host) = value.strip_prefix("https://") else {
        return false;
    };
    !host.is_empty()
        && !host.starts_with(['-', '.', ':'])
        && host.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-' || b == b':'
        })
}

fn is_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The ordered steps. Pure: revision, attempt and run directory are inputs, not lookups.
pub fn plan(
    workspace: &Workspace,
    request: &Request,
    revision: &str,
    run: &Path,
    attempt: &CutoverAttempt,
) -> Vec<Step> {
    let root = workspace.root();
    let local = |name: &'static str, program: &str, args: Vec<String>| Step {
        name,
        command: CommandSpec::new(program, root).args(args),
        remote_write: false,
    };
    let ssh = |name: &'static str, remote: String, remote_write: bool| Step {
        name,
        command: CommandSpec::new("ssh", root).args([
            "-o".to_owned(),
            "BatchMode=yes".to_owned(),
            request.host.clone(),
            remote,
        ]),
        remote_write,
    };
    let path = |name: &str| run.join(name).display().to_string();
    let incoming = incoming_directory(request, revision, attempt);
    let image = format!("{IMAGE_REPO}:{revision}");

    let source = vec![
        local(
            "source archive",
            "git",
            vec![
                "archive".into(),
                "--format=tar".into(),
                format!("--output={}", path("source.tar")),
                revision.into(),
            ],
        ),
        local(
            "source tree",
            "tar",
            vec![
                "-xf".into(),
                path("source.tar"),
                "-C".into(),
                path("context"),
            ],
        ),
    ];
    let bundle = local(
        "host bundle",
        "tar",
        vec![
            "-C".into(),
            path("context"),
            "-cf".into(),
            path("bundle.tar"),
            "REVISION".into(),
            "infra/vps".into(),
            "infra/edge/Caddyfile".into(),
            "services/cantos-api/schema/legacy-epoch-1-manifest.json".into(),
        ],
    );
    let upload_dir = ssh(
        "remote upload directory",
        format!("install -d -m 0700 {REMOTE_ROOT}/incoming {incoming}"),
        true,
    );
    let unpack = format!(
        "d={incoming}; install -d -m 0700 \"$d/tree\" && tar -xf \"$d/bundle.tar\" -C \"$d/tree\""
    );
    let install = format!("{unpack} && \"$d/tree/infra/vps/bin/cantos-vps\" install \"$d/tree\"");
    let smoke = request.smoke_origin.as_ref().map(|origin| {
        local(
            "public smoke",
            "curl",
            vec![
                "-fsS".into(),
                "-o".into(),
                "/dev/null".into(),
                "--max-time".into(),
                "20".into(),
                "-w".into(),
                "%{http_code} %{url_effective}\\n".into(),
                format!("{origin}/api/v1/theatre/works"),
            ],
        )
    });

    let mut steps = Vec::new();
    match request.operation {
        Operation::Deploy | Operation::CutoverEpoch2 | Operation::UpgradeEpoch2Prefix => {
            if request.operation.is_cutover() {
                steps.push(durability_step(workspace, run, "attempt"));
            }
            steps.extend(source);
            let context = path("context");
            let mut image_build = local(
                "image build",
                "podman",
                vec![
                    "build".into(),
                    "--pull=missing".into(),
                    "-f".into(),
                    format!("{context}/infra/vps/image/Containerfile"),
                    "--build-arg".into(),
                    format!("CANTOS_REVISION={revision}"),
                    "--label".into(),
                    format!("org.opencontainers.image.revision={revision}"),
                    "-t".into(),
                    image.clone(),
                    context,
                ],
            );
            if request.operation == Operation::UpgradeEpoch2Prefix {
                image_build
                    .command
                    .args
                    .insert(1, "--platform=linux/amd64".into());
            }
            steps.push(image_build);
            steps.push(local(
                "image export",
                "podman",
                vec![
                    "save".into(),
                    "--format=docker-archive".into(),
                    "-o".into(),
                    path("cantos-api.tar"),
                    image,
                ],
            ));
            steps.push(local(
                "image compress",
                "zstd",
                vec![
                    "-q".into(),
                    "-T0".into(),
                    "--rm".into(),
                    "-f".into(),
                    path("cantos-api.tar"),
                ],
            ));
            steps.push(bundle);
            steps.push(upload_dir);
            steps.push(Step {
                name: "upload",
                command: CommandSpec::new("scp", root).args([
                    "-q".to_owned(),
                    "-o".to_owned(),
                    "BatchMode=yes".to_owned(),
                    path("cantos-api.tar.zst"),
                    path("bundle.tar"),
                    format!("{}:{incoming}/", request.host),
                ]),
                remote_write: true,
            });
            steps.push(ssh(
                "remote image load",
                format!("zstd -dc {incoming}/cantos-api.tar.zst | podman load -q"),
                true,
            ));
            if request.operation.is_cutover() {
                let extracted = format!("d={incoming}; install -d -m 0700 \"$d/tree\" && tar -xf \"$d/bundle.tar\" -C \"$d/tree\"");
                steps.push(ssh("extract cutover bundle", extracted, true));
                let cutover = format!("{incoming}/tree/infra/vps/bin/cantos-vps");
                let protocol = request.operation.cutover_command();
                steps.push(ssh(
                    "remote cutover prepare",
                    format!("{cutover} {protocol}-prepare {revision} {incoming}/tree {attempt}"),
                    true,
                ));
                steps.push(Step {
                    name: "off-host recovery copy",
                    command: CommandSpec::new("scp", root).args([
                        "-q".into(),
                        "-o".into(),
                        "BatchMode=yes".into(),
                        format!("{}:{incoming}/cutover-backup.tar", request.host),
                        format!("{}:{incoming}/cutover-backup.tar.sha256", request.host),
                        run.display().to_string(),
                    ]),
                    remote_write: false,
                });
                steps.push(Step {
                    name: "verify off-host recovery copy",
                    command: CommandSpec::new("sha256sum", run).args([
                        "--check",
                        "--strict",
                        "cutover-backup.tar.sha256",
                    ]),
                    remote_write: false,
                });
                steps.push(durability_step(workspace, run, "offhost"));
                steps.push(Step {
                    name: "acknowledge verified off-host copy",
                    command: CommandSpec::new("scp", root).args([
                        "-q".into(),
                        "-o".into(),
                        "BatchMode=yes".into(),
                        path("cutover-backup.tar.sha256"),
                        format!("{}:{incoming}/cutover-offhost.sha256", request.host),
                    ]),
                    remote_write: true,
                });
                steps.push(ssh(
                    "remote cutover commit",
                    format!("{cutover} {protocol}-commit {revision} {incoming}/tree {attempt}"),
                    true,
                ));
            } else {
                steps.push(ssh("remote install", install, true));
                steps.push(ssh(
                    "remote deploy",
                    format!("{HOST_SCRIPT} deploy {revision}"),
                    true,
                ));
            }
            steps.extend(smoke);
        }
        Operation::Bootstrap | Operation::ReloadBrowser => {
            steps.extend(source);
            steps.push(bundle);
            steps.push(upload_dir);
            steps.push(Step {
                name: "upload",
                command: CommandSpec::new("scp", root).args([
                    "-q".to_owned(),
                    "-o".to_owned(),
                    "BatchMode=yes".to_owned(),
                    path("bundle.tar"),
                    format!("{}:{incoming}/", request.host),
                ]),
                remote_write: true,
            });
            if request.operation == Operation::ReloadBrowser {
                steps.push(ssh(
                    "remote browser reload",
                    format!(
                    "{unpack} && \"$d/tree/infra/vps/bin/cantos-vps\" reload-browser \"$d/tree\""
                ),
                    true,
                ));
            } else {
                steps.push(ssh("remote install", install, true));
                steps.push(ssh(
                    "remote bootstrap",
                    format!("{HOST_SCRIPT} bootstrap"),
                    true,
                ));
            }
            steps.extend(smoke);
        }
        Operation::Rollback => {
            steps.push(ssh(
                "remote rollback",
                format!("{HOST_SCRIPT} rollback"),
                true,
            ));
            steps.extend(smoke);
        }
        Operation::Status => {
            steps.push(ssh("remote status", format!("{HOST_SCRIPT} status"), false))
        }
    }
    steps
}

pub fn render_plan(
    workspace: &Workspace,
    request: &Request,
    revision: &str,
    steps: &[Step],
) -> String {
    let mut text = format!(
        "deploy vps cantos-api: {:?} on {}",
        request.operation, request.host
    );
    if is_revision(revision) {
        text.push_str(&format!(" at revision {revision}"));
    }
    text.push('\n');
    for (index, step) in steps.iter().enumerate() {
        let marker = if step.remote_write {
            "REMOTE WRITE"
        } else {
            "local/read"
        };
        text.push_str(&format!(
            "  {:>2}. [{marker}] {}\n      {}\n",
            index + 1,
            step.name,
            step.command.display(workspace.root())
        ));
    }
    if request.dry_run {
        if request.operation.is_cutover() {
            text.push_str("Cutover attempt is an all-zero planning placeholder; execution generates a fresh identity.\n");
        }
        text.push_str("Dry run: nothing was built, uploaded or run.\n");
    }
    text
}

const NEEDED: &[(Operation, &[&str])] = &[
    (
        Operation::Deploy,
        &["git", "tar", "podman", "zstd", "ssh", "scp"],
    ),
    (
        Operation::UpgradeEpoch2Prefix,
        &[
            "git",
            "tar",
            "podman",
            "zstd",
            "ssh",
            "scp",
            "sha256sum",
            "python3",
        ],
    ),
    (Operation::Bootstrap, &["git", "tar", "ssh", "scp"]),
    (Operation::ReloadBrowser, &["git", "tar", "ssh", "scp"]),
    (Operation::Rollback, &["ssh"]),
    (Operation::Status, &["ssh"]),
    (
        Operation::CutoverEpoch2,
        &[
            "git",
            "tar",
            "podman",
            "zstd",
            "ssh",
            "scp",
            "sha256sum",
            "python3",
        ],
    ),
];

pub fn execute(workspace: &Workspace, args: &VpsArgs, runner: &mut dyn Runner) -> Result<ExitCode> {
    let request = match resolve(args) {
        Ok(request) => request,
        Err(refusal) => return Ok(crate::refuse(refusal)),
    };
    if args.artifact.release_manifest.is_some() {
        if request.operation != Operation::Deploy {
            bail!("artifact promotion cannot bootstrap, rollback, cut over, reload or change identity");
        }
        let mut command = args
            .artifact
            .append(CommandSpec::new("python3", workspace.root()).args([
                "scripts/release_pipeline.py",
                "deploy-api",
                "--host",
                &request.host,
            ]));
        if let Some(origin) = &request.smoke_origin {
            command = command.args(["--api-origin", origin]);
        }
        command = command.arg_if(request.dry_run, "--dry-run");
        if request.dry_run {
            runner.run(&command)?;
            println!("NOT_RUN deploy vps: artifact plan only; no remote action");
            return Ok(ExitCode::SUCCESS);
        }
        let mut report = Report::new("deploy vps cantos-api artifact");
        match runner.run(&command) {
            Ok(()) => report.record(
                "artifact promotion",
                Status::Pass,
                "see component receipt; browser acceptance remains separate",
            ),
            Err(error) => report.record(
                "artifact promotion",
                Status::Fail,
                format!("{error:#}; inspect receipt and --status before retrying"),
            ),
        }
        report.print();
        return Ok(report.exit_code());
    }
    let needs_source = matches!(
        request.operation,
        Operation::Deploy
            | Operation::Bootstrap
            | Operation::ReloadBrowser
            | Operation::CutoverEpoch2
            | Operation::UpgradeEpoch2Prefix
    );
    let revision = if needs_source {
        head_revision(workspace.root())?
    } else {
        "-".repeat(40)
    };
    let attempt = if request.operation.is_cutover() && !request.dry_run {
        CutoverAttempt::generate()?
    } else {
        CutoverAttempt::PREVIEW
    };
    let attempt_suffix = if request.operation.is_cutover() {
        format!("-{attempt}")
    } else {
        String::new()
    };
    let run = workspace.path(&format!(
        "artifacts/vps/{}-{}{attempt_suffix}",
        utc_stamp(),
        &revision[..12.min(revision.len())]
    ));
    let steps = plan(workspace, &request, &revision, &run, &attempt);
    print!("{}", render_plan(workspace, &request, &revision, &steps));
    if request.dry_run {
        return Ok(ExitCode::SUCCESS);
    }
    let mut report = Report::new("deploy vps cantos-api");
    if request.operation == Operation::UpgradeEpoch2Prefix
        && !(cfg!(target_os = "linux") && cfg!(target_arch = "x86_64"))
    {
        report.record("native builder", Status::Unsupported, "prefix upgrade requires a native Linux x86_64 builder; no Mac/emulation or remote work attempted");
        report.print();
        return Ok(report.exit_code());
    }
    let needed = NEEDED
        .iter()
        .find(|(operation, _)| *operation == request.operation)
        .map_or(&[][..], |(_, tools)| *tools);
    let curl = request.smoke_origin.is_some().then_some("curl");
    let missing: Vec<&str> = needed
        .iter()
        .copied()
        .chain(curl)
        .filter(|tool| find_on_path(tool).is_none())
        .collect();
    if !missing.is_empty() {
        report.record(
            "local prerequisites",
            Status::Unsupported,
            format!("not on PATH: {}", missing.join(", ")),
        );
        report.print();
        return Ok(report.exit_code());
    }
    report.record("local prerequisites", Status::Pass, "");
    if needs_source {
        if dirty(workspace.root())? {
            report.note(format!(
                "the working tree has uncommitted changes; only commit {revision} is deployed"
            ));
        }
        std::fs::create_dir_all(run.join("context"))
            .with_context(|| format!("could not create {}", run.display()))?;
        #[cfg(unix)]
        if request.operation.is_cutover() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700))?;
        }
        if request.operation.is_cutover() {
            use std::io::Write;
            let mut record = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(run.join("cutover-attempt"))
                .context("could not create the immutable cutover attempt record")?;
            record.write_all(format!("{attempt}\n").as_bytes())?;
            record.sync_all()?;
            report.note(format!(
                "cutover attempt: {attempt}; retained for exact recovery"
            ));
        }
    }
    execute_plan(
        workspace,
        &request,
        &revision,
        &run,
        &attempt,
        runner,
        &mut report,
    )?;
    if needs_source {
        report.note(format!("run directory: {}", workspace.relative(&run)));
    }
    report.print();
    Ok(report.exit_code())
}

fn execute_plan(
    workspace: &Workspace,
    request: &Request,
    revision: &str,
    run: &Path,
    attempt: &CutoverAttempt,
    runner: &mut dyn Runner,
    report: &mut Report,
) -> Result<()> {
    let steps = plan(workspace, request, revision, run, attempt);
    let mut failed = false;
    let mut cutover_prepare_attempted = false;
    for step in &steps {
        if failed {
            report.record(step.name, Status::NotRun, "an earlier step failed");
            continue;
        }
        if step.name == "host bundle" {
            std::fs::write(run.join("context/REVISION"), format!("{revision}\n"))?;
        }
        if step.name == "remote cutover prepare" {
            cutover_prepare_attempted = true;
        }
        match runner.run(&step.command) {
            Ok(()) => report.record(step.name, Status::Pass, ""),
            Err(error) => {
                failed = true;
                let detail = if step.remote_write {
                    format!("{error:#}; the remote state may have changed: run --status before retrying")
                } else {
                    format!("{error:#}")
                };
                report.record(step.name, Status::Fail, detail);
            }
        }
    }
    if failed && cutover_prepare_attempted {
        // Recovery is state-aware: before acceptance it restores the paired legacy DB/image;
        // after acceptance it finalizes only, because the new database may have accepted writes.
        let incoming = incoming_directory(request, revision, attempt);
        let protocol = request.operation.cutover_command();
        let recovery = CommandSpec::new("ssh", workspace.root()).args([
            "-o".to_owned(), "BatchMode=yes".to_owned(), request.host.clone(),
            format!("{incoming}/tree/infra/vps/bin/cantos-vps {protocol}-recover {revision} {incoming}/tree {attempt}"),
        ]);
        match runner.run(&recovery) {
            Ok(()) => report.record("remote cutover recovery", Status::Pass, "state-aware recovery completed; original failure remains FAIL"),
            Err(error) => report.record("remote cutover recovery", Status::Fail, format!("{error:#}; stop and inspect the retained cutover journal before another operation")),
        }
    }
    Ok(())
}

fn head_revision(root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD^{commit}"])
        .current_dir(root)
        .output()
        .context("could not run git")?;
    let revision = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || !is_revision(&revision) {
        bail!("could not resolve HEAD to a commit");
    }
    Ok(revision)
}

fn dirty(root: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(root)
        .output()
        .context("could not run git")?;
    Ok(!output.stdout.is_empty())
}

fn utc_stamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = seconds / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let rem = seconds % 86_400;
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant's days-to-civil conversion (UTC calendar date from days since 1970-01-01).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const REV: &str = "0123456789abcdef0123456789abcdef01234567";
    const ATTEMPT: CutoverAttempt = CutoverAttempt([0x11; 16]);

    fn workspace() -> Workspace {
        Workspace::discover().unwrap()
    }

    fn args(extra: &[&str]) -> VpsArgs {
        use clap::Parser;
        #[derive(Parser)]
        struct Probe {
            #[command(flatten)]
            vps: VpsArgs,
        }
        let mut line = vec!["probe", "cantos-api", "--host", "root@203.0.113.10"];
        line.extend_from_slice(extra);
        Probe::try_parse_from(line).unwrap().vps
    }

    fn names(steps: &[Step]) -> Vec<&'static str> {
        steps.iter().map(|step| step.name).collect()
    }

    #[test]
    fn artifact_input_never_enters_source_build_or_operator_operations() {
        let mut raw = args(&["--smoke-origin", "https://api.example.org"]);
        raw.artifact = crate::release::ArtifactArgs {
            release_manifest: Some("/verified/manifest.json".into()),
            release_digest: Some("d".repeat(64)),
            release_sha: Some(REV.into()),
            local_artifact: true,
            ..Default::default()
        };
        let mut runner = crate::command::testing::RecordingRunner::default();
        assert_eq!(
            execute(&workspace(), &raw, &mut runner).unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(runner.commands.len(), 1);
        assert_eq!(runner.commands[0].program, "python3");
        assert!(runner.commands[0]
            .args
            .iter()
            .any(|arg| arg == "deploy-api"));
        for operation in [
            Operation::Bootstrap,
            Operation::ReloadBrowser,
            Operation::Rollback,
            Operation::Status,
            Operation::CutoverEpoch2,
            Operation::UpgradeEpoch2Prefix,
        ] {
            raw.bootstrap = operation == Operation::Bootstrap;
            raw.reload_browser = operation == Operation::ReloadBrowser;
            raw.rollback = operation == Operation::Rollback;
            raw.status = operation == Operation::Status;
            raw.cutover_epoch2 = operation == Operation::CutoverEpoch2;
            raw.upgrade_epoch2_prefix = operation == Operation::UpgradeEpoch2Prefix;
            raw.smoke_origin = None;
            let mut runner = crate::command::testing::RecordingRunner::default();
            assert!(execute(&workspace(), &raw, &mut runner).is_err());
            assert!(runner.commands.is_empty());
        }
    }

    #[test]
    fn deploy_builds_locally_before_any_remote_write_and_ends_in_the_host_gate() {
        let request = resolve(&args(&["--smoke-origin", "https://api.example.org"])).unwrap();
        assert_eq!(request.operation, Operation::Deploy);
        let run = PathBuf::from("/tmp/run");
        let steps = plan(&workspace(), &request, REV, &run, &ATTEMPT);
        assert_eq!(
            names(&steps),
            [
                "source archive",
                "source tree",
                "image build",
                "image export",
                "image compress",
                "host bundle",
                "remote upload directory",
                "upload",
                "remote image load",
                "remote install",
                "remote deploy",
                "public smoke",
            ]
        );
        let first_remote = steps.iter().position(|step| step.remote_write).unwrap();
        assert!(steps[..first_remote]
            .iter()
            .all(|step| step.command.program != "ssh"));
        assert_eq!(steps[first_remote].name, "remote upload directory");
        let build = &steps[2].command.args;
        assert!(build.contains(&format!("org.opencontainers.image.revision={REV}")));
        assert!(build.contains(&format!("localhost/cantos-api:{REV}")));
        assert!(build.contains(&"/tmp/run/context/infra/vps/image/Containerfile".to_owned()));
        assert_eq!(
            steps[0].command.args.last().unwrap(),
            REV,
            "archive the commit, not the tree"
        );
        let deploy = steps[10].command.args.last().unwrap();
        assert_eq!(deploy, &format!("/usr/local/sbin/cantos-vps deploy {REV}"));
        assert!(steps[10].command.args.contains(&"BatchMode=yes".to_owned()));
    }

    #[test]
    fn browser_reload_uploads_configuration_without_building_or_deploying_an_image() {
        let request = resolve(&args(&["--reload-browser"])).unwrap();
        assert_eq!(request.operation, Operation::ReloadBrowser);
        let steps = plan(&workspace(), &request, REV, Path::new("/tmp/run"), &ATTEMPT);
        assert_eq!(
            names(&steps),
            [
                "source archive",
                "source tree",
                "host bundle",
                "remote upload directory",
                "upload",
                "remote browser reload"
            ]
        );
        assert!(steps.iter().all(|step| step.command.program != "podman"));
        let remote = steps.last().unwrap().command.args.last().unwrap();
        assert!(remote.ends_with("reload-browser \"$d/tree\""));
        assert!(!remote.contains("cantos-vps\" install "));
    }

    #[test]
    fn bootstrap_uploads_only_the_host_bundle() {
        let request = resolve(&args(&["--bootstrap"])).unwrap();
        let steps = plan(&workspace(), &request, REV, Path::new("/tmp/run"), &ATTEMPT);
        assert!(steps.iter().all(|step| step.command.program != "podman"));
        assert_eq!(
            steps.last().unwrap().command.args.last().unwrap(),
            "/usr/local/sbin/cantos-vps bootstrap"
        );
        let upload = steps.iter().find(|step| step.name == "upload").unwrap();
        assert!(!upload
            .command
            .args
            .iter()
            .any(|arg| arg.contains("cantos-api.tar")));
    }

    #[test]
    fn epoch2_cutover_copies_and_verifies_recovery_off_host_before_commit() {
        let request = resolve(&args(&["--cutover-epoch2"])).unwrap();
        assert_eq!(request.operation, Operation::CutoverEpoch2);
        let steps = plan(
            &workspace(),
            &request,
            REV,
            Path::new("/tmp/private-run"),
            &ATTEMPT,
        );
        let names = names(&steps);
        assert!(!names.contains(&"remote install"));
        assert!(!names.contains(&"remote deploy"));
        let prepare = names
            .iter()
            .position(|name| *name == "remote cutover prepare")
            .unwrap();
        assert_eq!(
            &names[prepare..],
            [
                "remote cutover prepare",
                "off-host recovery copy",
                "verify off-host recovery copy",
                "persist off-host recovery copy",
                "acknowledge verified off-host copy",
                "remote cutover commit",
            ]
        );
        assert_eq!(
            steps[prepare + 2].command.cwd,
            Path::new("/tmp/private-run")
        );
        assert_eq!(steps[prepare + 2].command.program, "sha256sum");
        assert!(!steps[prepare + 1].remote_write);
        assert!(!steps[prepare + 2].remote_write);
        assert_eq!(steps.first().unwrap().name, "persist cutover attempt");
        assert_eq!(steps[prepare + 3].command.program, "python3");
        assert!(!steps[prepare + 3].remote_write);
        assert!(NEEDED
            .iter()
            .find(|(operation, _)| *operation == Operation::CutoverEpoch2)
            .unwrap()
            .1
            .contains(&"python3"));
        let commit = steps.last().unwrap().command.args.last().unwrap();
        assert_eq!(commit, &format!("/root/cantos-services/incoming/{REV}/{ATTEMPT}/tree/infra/vps/bin/cantos-vps cutover-commit {REV} /root/cantos-services/incoming/{REV}/{ATTEMPT}/tree {ATTEMPT}"));
        assert!(steps
            .iter()
            .find(|step| step.name == "host bundle")
            .unwrap()
            .command
            .args
            .contains(&"services/cantos-api/schema/legacy-epoch-1-manifest.json".to_owned()));
    }

    #[test]
    fn same_revision_cutovers_have_disjoint_remote_archives_acknowledgements_and_commands() {
        let request = resolve(&args(&["--cutover-epoch2"])).unwrap();
        let other = CutoverAttempt([0x22; 16]);
        let first = plan(
            &workspace(),
            &request,
            REV,
            Path::new("/tmp/first"),
            &ATTEMPT,
        );
        let second = plan(
            &workspace(),
            &request,
            REV,
            Path::new("/tmp/second"),
            &other,
        );
        for (steps, attempt, foreign) in [(&first, ATTEMPT, other), (&second, other, ATTEMPT)] {
            let incoming = format!("{REMOTE_ROOT}/incoming/{REV}/{attempt}");
            let all_arguments = steps
                .iter()
                .flat_map(|step| &step.command.args)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            assert!(all_arguments.contains(&format!("{incoming}/cutover-backup.tar")));
            assert!(all_arguments.contains(&format!("{incoming}/cutover-offhost.sha256")));
            assert!(!all_arguments.contains(&format!("{REMOTE_ROOT}/incoming/{REV}/{foreign}")));
            for name in ["remote cutover prepare", "remote cutover commit"] {
                let command = steps
                    .iter()
                    .find(|step| step.name == name)
                    .unwrap()
                    .command
                    .args
                    .last()
                    .unwrap();
                assert!(command.ends_with(&format!(" {REV} {incoming}/tree {attempt}")));
            }
        }
        assert_eq!(ATTEMPT.to_string(), "11111111111111111111111111111111");
    }

    #[cfg(unix)]
    struct LocalRun {
        root: PathBuf,
        workspace: Workspace,
        run: PathBuf,
    }

    #[cfg(unix)]
    impl LocalRun {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "cantos-vps-durability-{}-{}",
                std::process::id(),
                CutoverAttempt::generate().unwrap()
            ));
            let run = root.join("artifacts/vps/run");
            std::fs::create_dir_all(run.join("context")).unwrap();
            std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
            for name in [
                "cutover-attempt",
                "cutover-backup.tar",
                "cutover-backup.tar.sha256",
            ] {
                std::fs::write(run.join(name), "fixture bytes\n").unwrap();
            }
            let workspace = Workspace::fixture(&root).unwrap();
            Self {
                root,
                workspace,
                run,
            }
        }
    }

    #[cfg(unix)]
    impl Drop for LocalRun {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    #[test]
    fn durability_syncs_exact_files_and_the_new_directory_chain_through_workspace() {
        use std::os::unix::fs::MetadataExt;
        let fixture = LocalRun::new();
        let trace = "import json,os,sys\nobserved=[]\noriginal=os.fsync\ndef record(fd):\n s=os.fstat(fd); observed.append([s.st_dev,s.st_ino]); original(fd)\nos.fsync=record\nsource=sys.argv.pop(1)\nexec(source)\nprint(json.dumps(observed))\n";
        for (mode, files) in [
            ("attempt", vec!["cutover-attempt", "context"]),
            (
                "offhost",
                vec!["cutover-backup.tar", "cutover-backup.tar.sha256"],
            ),
        ] {
            let output = Command::new("python3")
                .args(["-c", trace, DURABLE_LOCAL_COPY, mode])
                .arg(&fixture.root)
                .arg(&fixture.run)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let observed: Vec<(u64, u64)> = serde_json::from_slice(&output.stdout).unwrap();
            let mut expected = files
                .iter()
                .map(|name| fixture.run.join(name))
                .collect::<Vec<_>>();
            let mut directory = fixture.run.as_path();
            loop {
                expected.push(directory.to_path_buf());
                if directory == fixture.root {
                    break;
                }
                directory = directory.parent().unwrap();
            }
            let expected = expected
                .iter()
                .map(|path| {
                    let metadata = path.metadata().unwrap();
                    (metadata.dev(), metadata.ino())
                })
                .collect::<Vec<_>>();
            assert_eq!(observed, expected, "fsync order for {mode}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn failed_durability_blocks_remote_ack_and_recovers_only_the_same_attempt() {
        #[derive(Default)]
        struct DurabilityRunner {
            commands: Vec<CommandSpec>,
        }
        impl Runner for DurabilityRunner {
            fn run(&mut self, command: &CommandSpec) -> Result<()> {
                self.commands.push(command.clone());
                if command.program == "python3" && !command.to_command().output()?.status.success()
                {
                    bail!("local durability fixture failed");
                }
                Ok(())
            }
        }
        for (missing, failed_stage, prepared) in [
            ("cutover-attempt", "persist cutover attempt", false),
            (
                "cutover-backup.tar.sha256",
                "persist off-host recovery copy",
                true,
            ),
        ] {
            let fixture = LocalRun::new();
            std::fs::remove_file(fixture.run.join(missing)).unwrap();
            let request = resolve(&args(&["--cutover-epoch2"])).unwrap();
            let mut runner = DurabilityRunner::default();
            let mut report = Report::new("durability fixture");
            execute_plan(
                &fixture.workspace,
                &request,
                REV,
                &fixture.run,
                &ATTEMPT,
                &mut runner,
                &mut report,
            )
            .unwrap();
            assert_eq!(report.exit_code(), ExitCode::FAILURE);
            assert_eq!(
                report
                    .outcomes
                    .iter()
                    .find(|row| row.name == failed_stage)
                    .unwrap()
                    .status,
                Status::Fail
            );
            for name in [
                "acknowledge verified off-host copy",
                "remote cutover commit",
            ] {
                assert_eq!(
                    report
                        .outcomes
                        .iter()
                        .find(|row| row.name == name)
                        .unwrap()
                        .status,
                    Status::NotRun
                );
            }
            let recovery = report
                .outcomes
                .iter()
                .find(|row| row.name == "remote cutover recovery");
            assert_eq!(
                recovery.map(|row| row.status),
                prepared.then_some(Status::Pass)
            );
            if prepared {
                let recovery = runner.commands.last().unwrap().args.last().unwrap();
                assert!(recovery.ends_with(&format!(
                    "cutover-recover {REV} {REMOTE_ROOT}/incoming/{REV}/{ATTEMPT}/tree {ATTEMPT}"
                )));
                assert!(!recovery.contains(&CutoverAttempt([0x22; 16]).to_string()));
            } else {
                assert!(runner
                    .commands
                    .iter()
                    .all(|command| command.program != "ssh" && command.program != "scp"));
            }
        }
    }

    #[test]
    fn epoch2_cutover_is_explicit_and_conflicts_with_other_operations() {
        use clap::Parser;
        #[derive(Parser)]
        struct Probe {
            #[command(flatten)]
            vps: VpsArgs,
        }
        for other in ["--status", "--rollback", "--bootstrap", "--reload-browser"] {
            assert!(Probe::try_parse_from([
                "probe",
                "cantos-api",
                "--host",
                "root@h",
                "--cutover-epoch2",
                other
            ])
            .is_err());
        }
        assert_eq!(resolve(&args(&[])).unwrap().operation, Operation::Deploy);
    }

    #[test]
    fn rollback_and_status_touch_only_the_host_script() {
        let rollback = plan(
            &workspace(),
            &resolve(&args(&["--rollback"])).unwrap(),
            REV,
            Path::new("/r"),
            &ATTEMPT,
        );
        assert_eq!(names(&rollback), ["remote rollback"]);
        assert!(rollback[0].remote_write);
        let status = plan(
            &workspace(),
            &resolve(&args(&["--status"])).unwrap(),
            REV,
            Path::new("/r"),
            &ATTEMPT,
        );
        assert_eq!(names(&status), ["remote status"]);
        assert!(!status[0].remote_write);
    }

    #[test]
    fn refuses_unsafe_destinations_and_origins() {
        for host in [
            "203.0.113.10",
            "-oProxyCommand=x@h",
            "root@-h",
            "root@h;id",
            "root@h p",
            "Root@h",
            "root@",
        ] {
            let mut raw = args(&[]);
            raw.host = host.into();
            assert!(matches!(resolve(&raw), Err(Refusal::Usage(_))), "{host}");
        }
        for origin in [
            "http://api.example.org",
            "https://api.example.org/",
            "https://a b",
            "https://",
        ] {
            let mut raw = args(&[]);
            raw.smoke_origin = Some(origin.into());
            assert!(matches!(resolve(&raw), Err(Refusal::Usage(_))), "{origin}");
        }
        assert!(matches!(
            resolve(&args(&[
                "--status",
                "--smoke-origin",
                "https://api.example.org"
            ])),
            Err(Refusal::Usage(_))
        ));
    }

    #[test]
    fn dry_run_plan_marks_remote_writes() {
        let request = resolve(&args(&["--dry-run"])).unwrap();
        let steps = plan(&workspace(), &request, REV, Path::new("/tmp/run"), &ATTEMPT);
        let text = render_plan(&workspace(), &request, REV, &steps);
        assert!(text.contains("[REMOTE WRITE] remote deploy"));
        assert!(text.contains("[local/read] image build"));
        assert!(text.ends_with("Dry run: nothing was built, uploaded or run.\n"));
    }

    #[test]
    fn civil_dates_are_exact() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_743), (2026, 10, 17));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }
    #[cfg(unix)]
    #[test]
    fn prefix_upgrade_uses_its_own_protocol_and_durable_attempt_bound_recovery() {
        let request = resolve(&args(&["--upgrade-epoch2-prefix"])).unwrap();
        assert_eq!(request.operation, Operation::UpgradeEpoch2Prefix);
        assert!(
            plan(&workspace(), &request, REV, Path::new("/tmp/run"), &ATTEMPT)
                .iter()
                .find(|step| step.name == "image build")
                .unwrap()
                .command
                .args
                .contains(&"--platform=linux/amd64".into())
        );
        let steps = plan(&workspace(), &request, REV, Path::new("/tmp/run"), &ATTEMPT);
        let prepare = steps
            .iter()
            .find(|step| step.name == "remote cutover prepare")
            .unwrap();
        assert!(prepare
            .command
            .args
            .last()
            .unwrap()
            .contains("prefix-prepare"));
        assert!(prepare
            .command
            .args
            .last()
            .unwrap()
            .contains(&format!("/{REV}/{ATTEMPT}/tree {ATTEMPT}")));
        let commit = steps
            .iter()
            .find(|step| step.name == "remote cutover commit")
            .unwrap();
        assert!(commit
            .command
            .args
            .last()
            .unwrap()
            .contains("prefix-commit"));
        assert!(!steps
            .iter()
            .any(|step| step.name == "remote install" || step.name == "remote deploy"));
        let order = names(&steps);
        assert!(
            order
                .iter()
                .position(|name| *name == "persist off-host recovery copy")
                .unwrap()
                < order
                    .iter()
                    .position(|name| *name == "acknowledge verified off-host copy")
                    .unwrap()
        );
        let mut runner = crate::command::testing::RecordingRunner {
            fail_on: Some(" prefix-commit ".into()),
            ..Default::default()
        };
        let mut report = Report::new("test prefix");
        let fixture = LocalRun::new();
        execute_plan(
            &fixture.workspace,
            &request,
            REV,
            &fixture.run,
            &ATTEMPT,
            &mut runner,
            &mut report,
        )
        .unwrap();
        assert!(runner
            .commands
            .last()
            .unwrap()
            .args
            .last()
            .unwrap()
            .contains("prefix-recover"));
    }

    #[test]
    fn prefix_upgrade_flag_conflicts_with_every_other_operator_operation() {
        use clap::Parser;
        #[derive(Parser)]
        struct Probe {
            #[command(flatten)]
            vps: VpsArgs,
        }
        for flag in [
            "--bootstrap",
            "--rollback",
            "--status",
            "--reload-browser",
            "--cutover-epoch2",
        ] {
            assert!(Probe::try_parse_from([
                "probe",
                "cantos-api",
                "--host",
                "root@203.0.113.10",
                "--upgrade-epoch2-prefix",
                flag
            ])
            .is_err());
        }
    }
}
