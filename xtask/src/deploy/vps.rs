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
  --rollback    switch the API back to the previous image on the VPS
  --status      print the VPS service status

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
    /// What to deploy; only the API runs on the VPS (Web apps go to Firebase Hosting).
    #[arg(value_enum)]
    pub target: VpsTarget,
    /// SSH destination, `user@host` (required; never defaulted).
    #[arg(long, value_name = "USER@HOST")]
    pub host: String,
    /// Upload the host bundle and run the idempotent host setup only.
    #[arg(long, conflicts_with_all = ["rollback", "status"])]
    pub bootstrap: bool,
    /// Switch the API back to the previously deployed image.
    #[arg(long, conflicts_with_all = ["bootstrap", "status"])]
    pub rollback: bool,
    /// Print the VPS service status (read-only).
    #[arg(long, conflicts_with_all = ["bootstrap", "rollback"])]
    pub status: bool,
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
    Rollback,
    Status,
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
    let operation = match (args.bootstrap, args.rollback, args.status) {
        (true, _, _) => Operation::Bootstrap,
        (_, true, _) => Operation::Rollback,
        (_, _, true) => Operation::Status,
        _ => Operation::Deploy,
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

/// The ordered steps. Pure: the revision and run directory are inputs, not lookups.
pub fn plan(workspace: &Workspace, request: &Request, revision: &str, run: &Path) -> Vec<Step> {
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
    let incoming = format!("{REMOTE_ROOT}/incoming/{revision}");
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
        ],
    );
    let upload_dir = ssh(
        "remote upload directory",
        format!("install -d -m 0700 {REMOTE_ROOT}/incoming {incoming}"),
        true,
    );
    let install = format!(
        "d={incoming}; install -d -m 0700 \"$d/tree\" && tar -xf \"$d/bundle.tar\" -C \"$d/tree\" && \"$d/tree/infra/vps/bin/cantos-vps\" install \"$d/tree\""
    );
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
        Operation::Deploy => {
            steps.extend(source);
            let context = path("context");
            steps.push(local(
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
            ));
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
            steps.push(ssh("remote install", install, true));
            steps.push(ssh(
                "remote deploy",
                format!("{HOST_SCRIPT} deploy {revision}"),
                true,
            ));
            steps.extend(smoke);
        }
        Operation::Bootstrap => {
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
            steps.push(ssh("remote install", install, true));
            steps.push(ssh(
                "remote bootstrap",
                format!("{HOST_SCRIPT} bootstrap"),
                true,
            ));
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
        "deploy vps cantos-api: {:?} on {} at revision {revision}\n",
        request.operation, request.host
    );
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
        text.push_str("Dry run: nothing was built, uploaded or run.\n");
    }
    text
}

const NEEDED: &[(Operation, &[&str])] = &[
    (
        Operation::Deploy,
        &["git", "tar", "podman", "zstd", "ssh", "scp"],
    ),
    (Operation::Bootstrap, &["git", "tar", "ssh", "scp"]),
    (Operation::Rollback, &["ssh"]),
    (Operation::Status, &["ssh"]),
];

pub fn execute(workspace: &Workspace, args: &VpsArgs, runner: &mut dyn Runner) -> Result<ExitCode> {
    let request = match resolve(args) {
        Ok(request) => request,
        Err(refusal) => return Ok(crate::refuse(refusal)),
    };
    let needs_source = matches!(request.operation, Operation::Deploy | Operation::Bootstrap);
    let revision = if needs_source {
        head_revision(workspace.root())?
    } else {
        "-".repeat(40)
    };
    let run = workspace.path(&format!(
        "artifacts/vps/{}-{}",
        utc_stamp(),
        &revision[..12.min(revision.len())]
    ));
    let steps = plan(workspace, &request, &revision, &run);
    print!("{}", render_plan(workspace, &request, &revision, &steps));
    if request.dry_run {
        return Ok(ExitCode::SUCCESS);
    }
    let mut report = Report::new("deploy vps cantos-api");
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
    }
    let mut failed = false;
    for step in &steps {
        if failed {
            report.record(step.name, Status::NotRun, "an earlier step failed");
            continue;
        }
        if step.name == "host bundle" {
            std::fs::write(run.join("context/REVISION"), format!("{revision}\n"))?;
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
    if needs_source {
        report.note(format!("run directory: {}", workspace.relative(&run)));
    }
    report.print();
    Ok(report.exit_code())
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
    fn deploy_builds_locally_before_any_remote_write_and_ends_in_the_host_gate() {
        let request = resolve(&args(&["--smoke-origin", "https://api.example.org"])).unwrap();
        assert_eq!(request.operation, Operation::Deploy);
        let run = PathBuf::from("/tmp/run");
        let steps = plan(&workspace(), &request, REV, &run);
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
    fn bootstrap_uploads_only_the_host_bundle() {
        let request = resolve(&args(&["--bootstrap"])).unwrap();
        let steps = plan(&workspace(), &request, REV, Path::new("/tmp/run"));
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
    fn rollback_and_status_touch_only_the_host_script() {
        let rollback = plan(
            &workspace(),
            &resolve(&args(&["--rollback"])).unwrap(),
            REV,
            Path::new("/r"),
        );
        assert_eq!(names(&rollback), ["remote rollback"]);
        assert!(rollback[0].remote_write);
        let status = plan(
            &workspace(),
            &resolve(&args(&["--status"])).unwrap(),
            REV,
            Path::new("/r"),
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
        let steps = plan(&workspace(), &request, REV, Path::new("/tmp/run"));
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
}
