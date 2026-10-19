//! The Wrangler contract: argv builders and pure interpreters of what Wrangler reports.
//!
//! The pinned release is [`PINNED_VERSION`], installed locally from the tracked lockfile in
//! `infra/cloudflare/wrangler/` (never `npx`, never a global install). Only three read/write
//! operations exist here: list projects (read), `pages deploy` (the one remote write) and list
//! deployments (read). xtask never logs in and never reads a credential: Wrangler reads
//! `CLOUDFLARE_API_TOKEN` from the environment itself.
//!
//! A deployment identity is accepted only from Wrangler's own machine-readable output file
//! (`WRANGLER_OUTPUT_FILE_PATH`, newline-delimited JSON, entry type `pages-deploy-detailed`) and is
//! validated against what was requested. An identity is never constructed, never taken from
//! "latest", and exit status 0 alone is never read as success.

use std::{path::Path, time::Duration};

use serde_json::Value;

use super::request::{AccountId, Branch, Environment, ProjectName};
use crate::{
    capture::{CaptureError, Captured},
    command::CommandSpec,
    firebase_cli,
};

/// The release the argv and output fixtures were taken from. Wrangler 4 requires Node >= 22.
// ACCEPTANCE: on 2026-10-19 a real preview deployment of each app with 4.151.2 produced the output-file
// entry, project listing and deployment listing these interpreters read (docs/deployment/cloudflare-pages.md).
// ACCEPTANCE(NOT_RUN): a production deployment, the error text of failure modes other than an invalid
// token, and any other Wrangler release. Owner: the operator of the first production deploy.
pub const PINNED_VERSION: &str = "4.151.2";
pub const WRANGLER_DIR: &str = "infra/cloudflare/wrangler";
pub const BINARY: &str = "infra/cloudflare/wrangler/node_modules/.bin/wrangler";
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(60);
pub const READ_TIMEOUT: Duration = Duration::from_secs(120);
pub const DEPLOY_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// Inherited variables that could redirect the account, API endpoint, configuration or
/// authentication scheme of a run. They are hidden from the child; the account is set explicitly.
const HIDDEN_ENVIRONMENT: [&str; 12] = [
    "CLOUDFLARE_API_BASE_URL",
    "CLOUDFLARE_API_KEY",
    "CLOUDFLARE_EMAIL",
    "CLOUDFLARE_ENV",
    "WRANGLER_API_ENVIRONMENT",
    "WRANGLER_AUTH_DOMAIN",
    "WRANGLER_AUTH_URL",
    "WRANGLER_CLIENT_ID",
    "WRANGLER_CF_AUTHORIZATION_TOKEN",
    "WRANGLER_OUTPUT_FILE_DIRECTORY",
    "WRANGLER_LOG_PATH",
    "WRANGLER_REGISTRY_PATH",
];

pub struct Invocation<'a> {
    pub binary: &'a Path,
    /// The run's own directory: it holds the staged `public/` and the output file, never a
    /// Wrangler configuration, `functions/` directory or `.env` file.
    pub cwd: &'a Path,
    pub account: &'a AccountId,
}

fn command(inv: &Invocation<'_>, args: &[&str]) -> CommandSpec {
    let mut spec = CommandSpec::new(inv.binary.display().to_string(), inv.cwd)
        .args(args.iter().copied())
        .env("CLOUDFLARE_ACCOUNT_ID", inv.account.as_str())
        .env("WRANGLER_SEND_METRICS", "false")
        .env("CI", "true");
    for name in HIDDEN_ENVIRONMENT {
        spec = spec.unset(name);
    }
    spec
}

pub fn version_command(inv: &Invocation<'_>) -> CommandSpec {
    command(inv, &["--version"])
}

pub fn projects_command(inv: &Invocation<'_>) -> CommandSpec {
    command(inv, &["pages", "project", "list", "--json"])
}

pub fn list_command(
    inv: &Invocation<'_>,
    project: &ProjectName,
    environment: Environment,
) -> CommandSpec {
    command(
        inv,
        &[
            "pages",
            "deployment",
            "list",
            "--project-name",
            project.as_str(),
            "--environment",
            environment.label(),
            "--json",
        ],
    )
}

pub struct Publish<'a> {
    pub project: &'a ProjectName,
    pub branch: &'a Branch,
    pub directory: &'a Path,
    pub commit_hash: &'a str,
    pub commit_message: &'a str,
    pub output_file: &'a Path,
}

/// The one remote write. `--commit-dirty=false` records that the uploaded bytes are an exact
/// release artifact, and `--skip-caching` makes Wrangler upload from this directory rather than
/// reuse a local cache. Wrangler is never handed a ZIP, the repository root or a `_worker.js`.
pub fn deploy_command(inv: &Invocation<'_>, publish: &Publish<'_>) -> CommandSpec {
    let directory = publish.directory.display().to_string();
    command(
        inv,
        &[
            "pages",
            "deploy",
            &directory,
            "--project-name",
            publish.project.as_str(),
            "--branch",
            publish.branch.as_str(),
            "--commit-hash",
            publish.commit_hash,
            "--commit-message",
            publish.commit_message,
            "--commit-dirty=false",
            "--skip-caching",
        ],
    )
    .env(
        "WRANGLER_OUTPUT_FILE_PATH",
        publish.output_file.display().to_string(),
    )
}

/// The first dotted number in the output: Wrangler may print a banner around the version.
pub fn parse_version(text: &str) -> Option<String> {
    firebase_cli::strip_ansi(text)
        .split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_digit() && c != '.'))
        .find(|token| {
            let parts: Vec<&str> = token.split('.').collect();
            parts.len() == 3
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        })
        .map(str::to_owned)
}

pub fn version_supported(found: &str) -> bool {
    found == PINNED_VERSION
}

fn keep_link(uri: &str) -> bool {
    let authority = uri
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .unwrap_or("");
    authority == "dash.cloudflare.com" || authority.ends_with(".pages.dev")
}

/// Redact child output for display or saving. Pages and dashboard links stay (public by design);
/// addresses, tokens and every other link do not.
pub fn redact(text: &str) -> String {
    let marked: Vec<String> = firebase_cli::strip_ansi(text)
        .lines()
        .map(|line| {
            line.split(' ')
                .map(|token| {
                    if looks_like_cloudflare_token(token) {
                        "[secret redacted]"
                    } else {
                        token
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    firebase_cli::redact(&marked.join("\n"), &[], &keep_link)
}

/// Cloudflare API tokens: the documented `cfut_`/`cfat_`/`cfk_` prefixes, or a long mixed-case
/// alphanumeric run (legacy 40-character tokens). A bare hexadecimal run (a commit SHA) is not.
fn looks_like_cloudflare_token(token: &str) -> bool {
    let core = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
    if ["cfut_", "cfat_", "cfk_"]
        .iter()
        .any(|prefix| core.starts_with(prefix))
    {
        return true;
    }
    core.len() >= 40
        && core
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        && core.bytes().any(|b| b.is_ascii_uppercase())
        && core.bytes().any(|b| b.is_ascii_lowercase())
        && core.bytes().any(|b| b.is_ascii_digit())
}

fn error_detail(captured: &Captured) -> String {
    // Wrangler prints `✘ [ERROR] <summary>` on stderr followed by the service's own reason.
    let clean = firebase_cli::strip_ansi(&captured.stderr);
    let lines: Vec<&str> = clean.lines().map(str::trim).collect();
    if let Some(index) = lines.iter().position(|line| line.contains("[ERROR]")) {
        let mut parts = vec![lines[index]];
        parts.extend(lines[index + 1..].iter().find(|line| !line.is_empty()));
        let joined: String = parts.join(" ").chars().take(240).collect();
        return redact(&joined);
    }
    firebase_cli::error_line(&captured.stderr, &captured.stdout, &[], &keep_link)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    Unavailable(String),
    Unreadable,
    NotFound,
    /// The project is connected to a Git provider. Direct Upload and Git integration are a
    /// choice made at project creation and do not convert; uploading here would not behave as
    /// planned.
    GitIntegrated,
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(why) => write!(f, "project lookup did not complete: {why}"),
            Self::Unreadable => f.write_str("the project listing was not recognised (Wrangler output may have changed)"),
            Self::NotFound => f.write_str("the project is not in this account; create it separately as a Direct Upload project (this command never creates one)"),
            Self::GitIntegrated => f.write_str("the project is Git-integrated, not Direct Upload"),
        }
    }
}

pub fn interpret_projects(
    result: Result<Captured, CaptureError>,
    project: &ProjectName,
) -> Result<Vec<String>, ProjectError> {
    let captured = result.map_err(|error| ProjectError::Unavailable(error.to_string()))?;
    if captured.code != 0 {
        return Err(ProjectError::Unavailable(format!(
            "exit {}: {}",
            captured.code,
            error_detail(&captured)
        )));
    }
    let document = json_array(&captured.stdout).ok_or(ProjectError::Unreadable)?;
    let mut found = None;
    for item in &document {
        let name = item
            .get("Project Name")
            .and_then(Value::as_str)
            .ok_or(ProjectError::Unreadable)?;
        if name == project.as_str() {
            found = Some(item);
        }
    }
    let item = found.ok_or(ProjectError::NotFound)?;
    match item.get("Git Provider").and_then(Value::as_str) {
        Some("No") => {}
        Some("Yes") => return Err(ProjectError::GitIntegrated),
        _ => return Err(ProjectError::Unreadable),
    }
    let domains = item
        .get("Project Domains")
        .and_then(Value::as_str)
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|domain| !domain.is_empty())
        .map(str::to_owned)
        .collect();
    Ok(domains)
}

/// The outermost JSON array in `text`; Wrangler may print notices around it.
fn json_array(text: &str) -> Option<Vec<Value>> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    (start < end)
        .then(|| serde_json::from_str::<Vec<Value>>(&text[start..=end]).ok())
        .flatten()
}

/// What a deployment is, read only from Wrangler's own result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Released {
    pub deployment_id: String,
    pub url: String,
    pub environment: String,
    pub production_branch: String,
    pub commit_hash: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectionKind {
    Authorization,
    ProjectNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownReason {
    TimedOut(Duration),
    Interrupted,
    OutputLimit,
    Plumbing(String),
    /// Exit 0, but no single recognised deployment identity.
    NoIdentity,
    /// More than one deployment entry: which one this run made is not established.
    AmbiguousIdentity,
    UnclassifiedFailure {
        code: i32,
    },
}

impl std::fmt::Display for UnknownReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimedOut(after) => write!(f, "Wrangler did not finish within {}s", after.as_secs()),
            Self::Interrupted => f.write_str("the run was interrupted"),
            Self::OutputLimit => f.write_str("Wrangler output exceeded the 4 MiB bound"),
            Self::Plumbing(why) => write!(f, "local capture failed: {why}"),
            Self::NoIdentity => f.write_str("Wrangler exited 0 but reported no recognised deployment (its output may have changed)"),
            Self::AmbiguousIdentity => f.write_str("Wrangler reported more than one deployment entry"),
            Self::UnclassifiedFailure { code } => write!(f, "Wrangler failed (exit {code}) in an unrecognised way, possibly after uploading"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteOutcome {
    /// No process ran; nothing was written.
    NotStarted(String),
    /// A recognised deployment of this project, branch, commit and environment.
    Confirmed { release: Released },
    /// A deployment exists but is not what was requested (for example a preview when production
    /// was intended). Its identity is known; nothing is rolled back or deleted automatically.
    Mismatch {
        release: Released,
        problems: Vec<String>,
    },
    /// A recognised refusal and no deployment identity.
    Rejected { kind: RejectionKind, detail: String },
    /// The write may have happened: read the target back before any retry.
    Unknown { reason: UnknownReason },
}

impl RemoteOutcome {
    pub fn release(&self) -> Option<&Released> {
        match self {
            Self::Confirmed { release } | Self::Mismatch { release, .. } => Some(release),
            _ => None,
        }
    }

    pub fn state(&self) -> &'static str {
        match self {
            Self::NotStarted(_) => "not_started",
            Self::Confirmed { .. } => "confirmed",
            Self::Mismatch { .. } => "mismatch",
            Self::Rejected { .. } => "rejected",
            Self::Unknown { .. } => "unknown",
        }
    }
}

/// What the interpreter must already know to accept a result as *this* deployment.
#[derive(Debug, Clone, Copy)]
pub struct Expected<'a> {
    pub project: &'a ProjectName,
    pub branch: &'a Branch,
    pub environment: Environment,
    pub commit_hash: &'a str,
}

/// `output_file` is the content of the file Wrangler was told to write, when it exists.
pub fn interpret_deploy(
    result: Result<Captured, CaptureError>,
    output_file: Option<&str>,
    timeout: Duration,
    expected: &Expected<'_>,
) -> RemoteOutcome {
    let captured = match result {
        Err(error) => {
            if !error.may_have_run() {
                return RemoteOutcome::NotStarted(error.to_string());
            }
            let reason = match error {
                CaptureError::TimedOut { .. } => UnknownReason::TimedOut(timeout),
                CaptureError::Interrupted { .. } => UnknownReason::Interrupted,
                CaptureError::OutputLimit { .. } => UnknownReason::OutputLimit,
                CaptureError::Plumbing { message, .. } => UnknownReason::Plumbing(message),
                CaptureError::Spawn { .. } => unreachable!("a failed spawn cannot have run"),
            };
            return RemoteOutcome::Unknown { reason };
        }
        Ok(captured) => captured,
    };
    let entries = detailed_entries(output_file.unwrap_or(""));
    if captured.code == 0 {
        return match entries.as_slice() {
            [] => RemoteOutcome::Unknown {
                reason: UnknownReason::NoIdentity,
            },
            [entry] => match read_release(entry, expected.project) {
                Some(release) => judge(release, expected),
                None => RemoteOutcome::Unknown {
                    reason: UnknownReason::NoIdentity,
                },
            },
            _ => RemoteOutcome::Unknown {
                reason: UnknownReason::AmbiguousIdentity,
            },
        };
    }
    // A failing exit with an identity is odd; the identity is not trusted, the write is unknown.
    if !entries.is_empty() {
        return RemoteOutcome::Unknown {
            reason: UnknownReason::UnclassifiedFailure {
                code: captured.code,
            },
        };
    }
    let text = firebase_cli::strip_ansi(&format!("{}\n{}", captured.stdout, captured.stderr))
        .to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|needle| text.contains(needle));
    let kind = if has(&[
        "authentication error",
        "invalid api token",
        "invalid access token",
        "[code: 10000]",
        "[code: 9106]",
    ]) {
        Some(RejectionKind::Authorization)
    } else if has(&["project not found", "[code: 8000007]"]) {
        Some(RejectionKind::ProjectNotFound)
    } else {
        None
    };
    match kind {
        Some(kind) => RemoteOutcome::Rejected {
            kind,
            detail: error_detail(&captured),
        },
        None => RemoteOutcome::Unknown {
            reason: UnknownReason::UnclassifiedFailure {
                code: captured.code,
            },
        },
    }
}

fn detailed_entries(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("pages-deploy-detailed"))
        .collect()
}

fn is_deployment_id(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(index, b)| match index {
            8 | 13 | 18 | 23 => b == b'-',
            _ => matches!(b, b'0'..=b'9' | b'a'..=b'f'),
        })
}

/// A `*.pages.dev` URL of this project: `https://<one label>.<project>.pages.dev`, or the
/// project's own `https://<project>.pages.dev`.
fn is_project_url(url: &str, project: &ProjectName) -> bool {
    let Some(host) = url.strip_prefix("https://") else {
        return false;
    };
    let host = host.trim_end_matches('/');
    if host.contains(['/', '?', '#', '@', ':']) {
        return false;
    }
    let suffix = format!(".{}.pages.dev", project.as_str());
    let own = format!("{}.pages.dev", project.as_str());
    host == own
        || host.strip_suffix(&suffix).is_some_and(|label| {
            !label.is_empty()
                && !label.contains('.')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

fn read_release(entry: &Value, project: &ProjectName) -> Option<Released> {
    if entry.get("version").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let text = |key: &str| entry.get(key).and_then(Value::as_str);
    if text("pages_project")? != project.as_str() {
        return None;
    }
    let deployment_id = text("deployment_id")
        .filter(|id| is_deployment_id(id))?
        .to_owned();
    let url = text("url")
        .filter(|url| is_project_url(url, project))?
        .to_owned();
    let environment = text("environment")
        .filter(|e| matches!(*e, "production" | "preview"))?
        .to_owned();
    let production_branch = text("production_branch")?.to_owned();
    let commit_hash = entry
        .get("deployment_trigger")
        .and_then(|t| t.get("metadata"))
        .and_then(|m| m.get("commit_hash"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let alias = text("alias")
        .filter(|alias| is_project_url(alias, project))
        .map(str::to_owned);
    Some(Released {
        deployment_id,
        url,
        environment,
        production_branch,
        commit_hash,
        alias,
    })
}

fn judge(release: Released, expected: &Expected<'_>) -> RemoteOutcome {
    let mut problems = Vec::new();
    if release.environment != expected.environment.label() {
        problems.push(format!(
            "the deployment is `{}`, but `{}` was requested",
            release.environment,
            expected.environment.label()
        ));
    }
    let on_production_branch = release.production_branch == expected.branch.as_str();
    if on_production_branch != (expected.environment == Environment::Production) {
        problems.push(format!(
            "the project's production branch is `{}`, which disagrees with the requested branch `{}` and environment",
            release.production_branch,
            expected.branch.as_str()
        ));
    }
    if release.commit_hash != expected.commit_hash {
        problems.push("the deployment's commit hash is not the artifact's source SHA".to_owned());
    }
    if problems.is_empty() {
        RemoteOutcome::Confirmed { release }
    } else {
        RemoteOutcome::Mismatch { release, problems }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readback {
    /// The deployment ID is listed in the requested environment.
    Listed,
    /// The listing completed and does not contain it.
    Absent,
    /// The listing could not be read; the deployment is neither confirmed nor excluded.
    Unavailable(String),
}

/// Look for `deployment_id` in a `pages deployment list --json` result.
pub fn interpret_list(
    result: Result<Captured, CaptureError>,
    deployment_id: &str,
    environment: Environment,
) -> Readback {
    let captured = match result {
        Ok(captured) if captured.code == 0 => captured,
        Ok(captured) => {
            return Readback::Unavailable(format!(
                "exit {}: {}",
                captured.code,
                error_detail(&captured)
            ))
        }
        Err(error) => return Readback::Unavailable(error.to_string()),
    };
    let Some(items) = json_array(&captured.stdout) else {
        return Readback::Unavailable("the listing was not recognised".to_owned());
    };
    let wanted = capitalize(environment.label());
    let listed = items.iter().any(|item| {
        item.get("Id").and_then(Value::as_str) == Some(deployment_id)
            && item.get("Environment").and_then(Value::as_str) == Some(wanted.as_str())
    });
    if listed {
        Readback::Listed
    } else {
        Readback::Absent
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
}

/// After an unknown outcome: the listed deployments of this branch, short commit and
/// environment. The listing alone cannot say which of them this run created, so it is evidence
/// for the operator, never an identity.
pub fn interpret_candidates(
    result: Result<Captured, CaptureError>,
    branch: &str,
    short_commit: &str,
    environment: Environment,
) -> Result<Vec<Candidate>, String> {
    let captured = result.map_err(|error| error.to_string())?;
    if captured.code != 0 {
        return Err(format!(
            "exit {}: {}",
            captured.code,
            error_detail(&captured)
        ));
    }
    let items =
        json_array(&captured.stdout).ok_or_else(|| "the listing was not recognised".to_owned())?;
    let wanted = capitalize(environment.label());
    Ok(items
        .iter()
        .filter(|item| {
            item.get("Branch").and_then(Value::as_str) == Some(branch)
                && item.get("Source").and_then(Value::as_str) == Some(short_commit)
                && item.get("Environment").and_then(Value::as_str) == Some(wanted.as_str())
        })
        .filter_map(|item| item.get("Id").and_then(Value::as_str))
        .filter(|id| is_deployment_id(id))
        .map(|id| Candidate { id: id.to_owned() })
        .collect())
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
pub(super) mod fixtures {
    pub const ID: &str = "0f6d9a52-2b0c-4a6b-9c1d-3e4f5a6b7c8d";
    pub const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    /// A sanitized entry in the shape of the 4.151.2 `pages-deploy-detailed` output record.
    pub fn deploy_entry(
        project: &str,
        environment: &str,
        production_branch: &str,
        commit: &str,
    ) -> String {
        format!(
            "{{\"type\":\"pages-deploy\",\"version\":1,\"pages_project\":\"{project}\",\"deployment_id\":\"{ID}\",\"url\":\"https://0f6d9a52.{project}.pages.dev\"}}\n\
             {{\"type\":\"pages-deploy-detailed\",\"version\":1,\"pages_project\":\"{project}\",\"deployment_id\":\"{ID}\",\"url\":\"https://0f6d9a52.{project}.pages.dev\",\"alias\":\"https://main.{project}.pages.dev\",\"environment\":\"{environment}\",\"production_branch\":\"{production_branch}\",\"deployment_trigger\":{{\"metadata\":{{\"commit_hash\":\"{commit}\"}}}}}}\n"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    fn project() -> ProjectName {
        ProjectName::parse("cantos-theatre").unwrap()
    }

    fn branch(name: &str) -> Branch {
        Branch::parse(name).unwrap()
    }

    fn ok(stdout: &str) -> Result<Captured, CaptureError> {
        Ok(Captured {
            code: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        })
    }

    fn failed(code: i32, stderr: &str) -> Result<Captured, CaptureError> {
        Ok(Captured {
            code,
            stdout: String::new(),
            stderr: stderr.into(),
        })
    }

    fn inv<'a>(account: &'a AccountId, binary: &'a Path, cwd: &'a Path) -> Invocation<'a> {
        Invocation {
            binary,
            cwd,
            account,
        }
    }

    #[test]
    fn the_deploy_argv_names_every_destination_explicitly_and_nothing_else() {
        let account = AccountId::parse("8339afe44b4e8cd2b633a38a77f1db41").unwrap();
        let (project, main) = (project(), branch("main"));
        let spec = deploy_command(
            &inv(&account, Path::new("/r/wrangler"), Path::new("/run")),
            &Publish {
                project: &project,
                branch: &main,
                directory: Path::new("/run/public"),
                commit_hash: SHA,
                commit_message: "cantos theatre RUN",
                output_file: Path::new("/run/out/wrangler.ndjson"),
            },
        );
        assert_eq!(spec.program, "/r/wrangler");
        assert_eq!(spec.cwd, Path::new("/run"));
        assert_eq!(
            spec.args,
            [
                "pages",
                "deploy",
                "/run/public",
                "--project-name",
                "cantos-theatre",
                "--branch",
                "main",
                "--commit-hash",
                SHA,
                "--commit-message",
                "cantos theatre RUN",
                "--commit-dirty=false",
                "--skip-caching"
            ]
        );
        assert!(!spec
            .args
            .iter()
            .any(|a| a == "npx" || a.contains("login") || a.contains("config")));
        let env = |key: &str| {
            spec.env
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(
            env("CLOUDFLARE_ACCOUNT_ID"),
            Some("8339afe44b4e8cd2b633a38a77f1db41")
        );
        assert_eq!(
            env("WRANGLER_OUTPUT_FILE_PATH"),
            Some("/run/out/wrangler.ndjson")
        );
        assert_eq!(env("WRANGLER_SEND_METRICS"), Some("false"));
        assert!(spec
            .env_remove
            .iter()
            .any(|k| k == "CLOUDFLARE_API_BASE_URL"));
        assert!(spec
            .env_remove
            .iter()
            .any(|k| k == "WRANGLER_API_ENVIRONMENT"));
        assert!(
            !spec.env.iter().any(|(k, _)| k.contains("TOKEN")),
            "the token is never copied into argv or env here"
        );
    }

    #[test]
    fn the_read_commands_never_write_and_pin_project_and_environment() {
        let account = AccountId::parse("8339afe44b4e8cd2b633a38a77f1db41").unwrap();
        let i = inv(&account, Path::new("/r/wrangler"), Path::new("/run"));
        assert_eq!(
            projects_command(&i).args,
            ["pages", "project", "list", "--json"]
        );
        assert_eq!(
            list_command(&i, &project(), Environment::Preview).args,
            [
                "pages",
                "deployment",
                "list",
                "--project-name",
                "cantos-theatre",
                "--environment",
                "preview",
                "--json"
            ]
        );
        assert_eq!(version_command(&i).args, ["--version"]);
    }

    #[test]
    fn the_version_is_found_in_a_banner_and_only_the_pinned_one_is_supported() {
        assert_eq!(parse_version("4.151.2\n").as_deref(), Some("4.151.2"));
        assert_eq!(
            parse_version("\u{1b}[1m⛅️ wrangler 4.151.2\u{1b}[0m\n────\n").as_deref(),
            Some("4.151.2")
        );
        assert_eq!(parse_version("no version"), None);
        assert!(version_supported("4.151.2"));
        assert!(!version_supported("4.151.3"));
        assert!(!version_supported("3.114.0"));
    }

    const LISTING: &str = "[{\"Project Name\":\"cantos-theatre\",\"Project Domains\":\"cantos-theatre.pages.dev, cantos.cc\",\"Git Provider\":\"No\",\"Last Modified\":\"1 day ago\"},{\"Project Name\":\"cantos-studio\",\"Project Domains\":\"cantos-studio.pages.dev\",\"Git Provider\":\"Yes\",\"Last Modified\":\"1 day ago\"}]";

    #[test]
    fn a_direct_upload_project_in_the_listing_is_accepted_with_its_domains() {
        let domains = interpret_projects(ok(LISTING), &project()).unwrap();
        assert_eq!(domains, ["cantos-theatre.pages.dev", "cantos.cc"]);
    }

    #[test]
    fn a_missing_git_integrated_or_unreadable_project_stops_before_upload() {
        let studio = ProjectName::parse("cantos-studio").unwrap();
        assert_eq!(
            interpret_projects(ok(LISTING), &studio),
            Err(ProjectError::GitIntegrated)
        );
        let other = ProjectName::parse("someone-else").unwrap();
        assert_eq!(
            interpret_projects(ok(LISTING), &other),
            Err(ProjectError::NotFound)
        );
        assert_eq!(
            interpret_projects(ok("[]"), &project()),
            Err(ProjectError::NotFound)
        );
        assert_eq!(
            interpret_projects(ok("no json"), &project()),
            Err(ProjectError::Unreadable)
        );
        assert_eq!(
            interpret_projects(ok("[{\"x\":1}]"), &project()),
            Err(ProjectError::Unreadable)
        );
        assert_eq!(
            interpret_projects(
                ok("[{\"Project Name\":\"cantos-theatre\",\"Git Provider\":\"Maybe\"}]"),
                &project()
            ),
            Err(ProjectError::Unreadable)
        );
        assert!(matches!(
            interpret_projects(failed(1, "Error: 10000 Authentication error"), &project()),
            Err(ProjectError::Unavailable(_))
        ));
    }

    fn expect<'a>(
        project: &'a ProjectName,
        branch: &'a Branch,
        environment: Environment,
    ) -> Expected<'a> {
        Expected {
            project,
            branch,
            environment,
            commit_hash: SHA,
        }
    }

    #[test]
    fn a_recognised_matching_deployment_is_confirmed_from_the_output_file_only() {
        let (project, main) = (project(), branch("main"));
        let file = deploy_entry("cantos-theatre", "production", "main", SHA);
        let outcome = interpret_deploy(
            ok("✨ Deployment complete!"),
            Some(&file),
            DEPLOY_TIMEOUT,
            &expect(&project, &main, Environment::Production),
        );
        let RemoteOutcome::Confirmed { release } = outcome else {
            panic!("{outcome:?}")
        };
        assert_eq!(release.deployment_id, ID);
        assert_eq!(release.url, "https://0f6d9a52.cantos-theatre.pages.dev");
        assert_eq!(
            release.alias.as_deref(),
            Some("https://main.cantos-theatre.pages.dev")
        );
    }

    #[test]
    fn exit_zero_without_an_identity_is_unknown_never_success() {
        let (project, main) = (project(), branch("main"));
        let expected = expect(&project, &main, Environment::Production);
        for file in [
            None,
            Some(""),
            Some("not json\n"),
            Some("{\"type\":\"pages-deploy\",\"version\":1}\n"),
        ] {
            let outcome = interpret_deploy(
                ok("Deployment complete https://0f6d9a52.cantos-theatre.pages.dev"),
                file,
                DEPLOY_TIMEOUT,
                &expected,
            );
            assert_eq!(
                outcome,
                RemoteOutcome::Unknown {
                    reason: UnknownReason::NoIdentity
                },
                "{file:?}"
            );
        }
    }

    #[test]
    fn an_unrecognised_shape_or_a_foreign_identity_is_never_accepted() {
        let (project, main) = (project(), branch("main"));
        let expected = expect(&project, &main, Environment::Production);
        let good = deploy_entry("cantos-theatre", "production", "main", SHA);
        let mutations = [
            good.replace(
                "\"version\":1,\"pages_project\":\"cantos-theatre\",\"deployment_id\"",
                "\"version\":2,\"pages_project\":\"cantos-theatre\",\"deployment_id\"",
            ),
            good.replace(
                "\"pages_project\":\"cantos-theatre\"",
                "\"pages_project\":\"cantos-studio\"",
            ),
            good.replace(ID, "latest"),
            good.replace(
                "https://0f6d9a52.cantos-theatre.pages.dev",
                "https://evil.example/x",
            ),
            good.replace(
                "https://0f6d9a52.cantos-theatre.pages.dev",
                "https://a.b.cantos-theatre.pages.dev",
            ),
            good.replace(
                "\"environment\":\"production\"",
                "\"environment\":\"staging\"",
            ),
        ];
        for (index, file) in mutations.iter().enumerate() {
            let outcome = interpret_deploy(ok(""), Some(file), DEPLOY_TIMEOUT, &expected);
            assert!(
                !matches!(outcome, RemoteOutcome::Confirmed { .. }),
                "mutation {index}: {outcome:?}"
            );
        }
        let two = format!("{good}{good}");
        assert_eq!(
            interpret_deploy(ok(""), Some(&two), DEPLOY_TIMEOUT, &expected),
            RemoteOutcome::Unknown {
                reason: UnknownReason::AmbiguousIdentity
            }
        );
    }

    #[test]
    fn a_preview_when_production_was_intended_is_a_mismatch_with_its_identity_kept() {
        let (project, main) = (project(), branch("main"));
        let file = deploy_entry("cantos-theatre", "preview", "develop", SHA);
        let outcome = interpret_deploy(
            ok(""),
            Some(&file),
            DEPLOY_TIMEOUT,
            &expect(&project, &main, Environment::Production),
        );
        let RemoteOutcome::Mismatch { release, problems } = outcome else {
            panic!("{outcome:?}")
        };
        assert_eq!(release.deployment_id, ID);
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    #[test]
    fn a_wrong_commit_hash_is_a_mismatch() {
        let (project, feature) = (project(), branch("review/x"));
        let file = deploy_entry("cantos-theatre", "preview", "main", &"f".repeat(40));
        let outcome = interpret_deploy(
            ok(""),
            Some(&file),
            DEPLOY_TIMEOUT,
            &expect(&project, &feature, Environment::Preview),
        );
        let RemoteOutcome::Mismatch { problems, .. } = outcome else {
            panic!("{outcome:?}")
        };
        assert!(problems.iter().any(|p| p.contains("commit hash")));
    }

    #[test]
    fn recognised_refusals_without_an_identity_are_rejected_everything_else_is_unknown() {
        let (project, main) = (project(), branch("main"));
        let expected = expect(&project, &main, Environment::Production);
        let auth = interpret_deploy(failed(1, "✘ [ERROR] A request to the Cloudflare API failed. Authentication error [code: 10000]"), None, DEPLOY_TIMEOUT, &expected);
        assert!(
            matches!(
                auth,
                RemoteOutcome::Rejected {
                    kind: RejectionKind::Authorization,
                    ..
                }
            ),
            "{auth:?}"
        );
        let missing = interpret_deploy(
            failed(1, "Project not found. [code: 8000007]"),
            None,
            DEPLOY_TIMEOUT,
            &expected,
        );
        assert!(matches!(
            missing,
            RemoteOutcome::Rejected {
                kind: RejectionKind::ProjectNotFound,
                ..
            }
        ));
        let other = interpret_deploy(
            failed(1, "fetch failed: socket hang up"),
            None,
            DEPLOY_TIMEOUT,
            &expected,
        );
        assert_eq!(
            other,
            RemoteOutcome::Unknown {
                reason: UnknownReason::UnclassifiedFailure { code: 1 }
            }
        );
        let identity_with_failure = deploy_entry("cantos-theatre", "production", "main", SHA);
        assert_eq!(
            interpret_deploy(
                failed(1, "boom"),
                Some(&identity_with_failure),
                DEPLOY_TIMEOUT,
                &expected
            ),
            RemoteOutcome::Unknown {
                reason: UnknownReason::UnclassifiedFailure { code: 1 }
            }
        );
    }

    #[test]
    fn timeouts_interruptions_and_overflow_after_start_are_unknown_and_a_failed_spawn_is_not_started(
    ) {
        let (project, main) = (project(), branch("main"));
        let expected = expect(&project, &main, Environment::Production);
        let timeout = interpret_deploy(
            Err(CaptureError::TimedOut {
                after: Duration::from_secs(5),
                partial: Default::default(),
            }),
            None,
            DEPLOY_TIMEOUT,
            &expected,
        );
        assert_eq!(
            timeout,
            RemoteOutcome::Unknown {
                reason: UnknownReason::TimedOut(DEPLOY_TIMEOUT)
            }
        );
        let interrupted = interpret_deploy(
            Err(CaptureError::Interrupted {
                partial: Default::default(),
                started: true,
            }),
            None,
            DEPLOY_TIMEOUT,
            &expected,
        );
        assert_eq!(interrupted.state(), "unknown");
        let overflow = interpret_deploy(
            Err(CaptureError::OutputLimit {
                partial: Default::default(),
            }),
            None,
            DEPLOY_TIMEOUT,
            &expected,
        );
        assert_eq!(overflow.state(), "unknown");
        let spawn = interpret_deploy(
            Err(CaptureError::Spawn {
                program: "wrangler".into(),
                message: "ENOENT".into(),
            }),
            None,
            DEPLOY_TIMEOUT,
            &expected,
        );
        assert_eq!(spawn.state(), "not_started");
        let not_run = interpret_deploy(
            Err(CaptureError::Interrupted {
                partial: Default::default(),
                started: false,
            }),
            None,
            DEPLOY_TIMEOUT,
            &expected,
        );
        assert_eq!(not_run.state(), "not_started");
    }

    #[test]
    fn readback_finds_the_exact_id_in_the_exact_environment() {
        let listing = format!("[{{\"Id\":\"{ID}\",\"Environment\":\"Production\",\"Branch\":\"main\",\"Source\":\"0123456\",\"Deployment\":\"https://0f6d9a52.cantos-theatre.pages.dev\",\"Status\":\"1 minute ago\",\"Build\":\"https://dash.cloudflare.com/x\"}}]");
        assert_eq!(
            interpret_list(ok(&listing), ID, Environment::Production),
            Readback::Listed
        );
        assert_eq!(
            interpret_list(ok(&listing), ID, Environment::Preview),
            Readback::Absent
        );
        assert_eq!(
            interpret_list(ok(&listing), "other", Environment::Production),
            Readback::Absent
        );
        assert!(matches!(
            interpret_list(ok("garbage"), ID, Environment::Production),
            Readback::Unavailable(_)
        ));
        assert!(matches!(
            interpret_list(failed(1, "x"), ID, Environment::Production),
            Readback::Unavailable(_)
        ));
    }

    #[test]
    fn a_real_authentication_failure_surfaces_the_error_not_the_progress_line() {
        // Sanitized from a real Wrangler 4.151.2 run with an invalid token (nothing was uploaded).
        let stdout = "\n📎 It looks like you are authenticating Wrangler via a custom API token set in an environment variable.\nPlease ensure it has the correct permissions for this operation.\n\nGetting User settings...\n";
        let stderr = "\u{1b}[31m✘ \u{1b}[41;31m[\u{1b}[41;97mERROR\u{1b}[41;31m]\u{1b}[0m \u{1b}[1mA request to the Cloudflare API (/accounts/ACCOUNT/pages/projects) failed.\u{1b}[0m\n\n  Authentication failed (status: 400) [code: 9106]\n\n\n\u{1b}[31m✘ [ERROR] A request to the Cloudflare API (/user/tokens/verify) failed.\n";
        let result = Ok(Captured {
            code: 1,
            stdout: stdout.into(),
            stderr: stderr.into(),
        });
        let Err(ProjectError::Unavailable(detail)) = interpret_projects(result, &project()) else {
            panic!()
        };
        assert!(
            detail.contains("Authentication failed (status: 400) [code: 9106]"),
            "{detail}"
        );
        assert!(!detail.contains("Getting User settings"));
        let (p, main) = (project(), branch("main"));
        let again = Ok(Captured {
            code: 1,
            stdout: stdout.into(),
            stderr: stderr.into(),
        });
        let outcome = interpret_deploy(
            again,
            None,
            DEPLOY_TIMEOUT,
            &expect(&p, &main, Environment::Production),
        );
        assert!(
            matches!(
                outcome,
                RemoteOutcome::Rejected {
                    kind: RejectionKind::Authorization,
                    ..
                }
            ),
            "{outcome:?}"
        );
    }

    #[test]
    fn redaction_keeps_pages_and_dashboard_links_and_removes_secrets_and_other_links() {
        let text = "deployed https://0f6d9a52.cantos-theatre.pages.dev see https://dash.cloudflare.com/acct/pages token cfut_abcdefghijklmnopqrstuvwxyz0123456789ABCD mail a@b.example https://evil.example/x";
        let clean = redact(text);
        assert!(clean.contains("https://0f6d9a52.cantos-theatre.pages.dev"));
        assert!(clean.contains("https://dash.cloudflare.com/acct/pages"));
        assert!(!clean.contains("evil.example"));
        assert!(!clean.contains("a@b.example"));
        assert!(!clean.contains("cfut_abcdefghijklmnopqrstuvwxyz"));
    }
}
