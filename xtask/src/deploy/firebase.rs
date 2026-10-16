//! The Firebase Hosting adapter, as pure functions over captured output. It names the CLI contract
//! it was written against, builds argv without a shell and translates each process result into a
//! closed vocabulary that says what is *known*.
//!
//! The contract is `firebase-tools` **15.x, tested against 15.34.0** (the same pin as
//! `distribute firebase`; the Hosting commands are identical in 15.33.0). It is read from the CLI
//! sources, not from a live run:
//! - with `--json` the CLI prints nothing else to stdout: one `{"status":"success","result":...}`
//!   or `{"status":"error","error":"..."}` document, and exits nonzero on error;
//! - `hosting:channel:deploy` creates (or re-times) the preview channel first, then uploads, then
//!   finalizes and releases. Its result is keyed by target: `{target, site, url, version,
//!   expireTime}`. It refuses `live` and silently rewrites `/ : _ #` in the channel ID;
//! - `deploy --only hosting:<target>` returns `{hosting: "<version resource name>"}` for one
//!   deploy and no URL; the live URL comes from the read-only site lookup;
//! - `hosting:sites:list` returns `{sites: [...]}`, each a Hosting `Site` whose `name` is
//!   `projects/<id-or-number>/sites/<site>`.
//!
//! Because a failed run may already have created a preview channel or uploaded an unreleased
//! version, a nonzero exit is *never* reported as "nothing was written". The interpreter is
//! fail-closed: anything it does not recognise after the write may have started is an *unknown*
//! remote outcome, never a success.

// ACCEPTANCE(NOT_RUN): every response shape below is reconstructed from the CLI sources and the
// Hosting API reference (`xtask/fixtures/firebase-tools-15.34.0/hosting-*.json`); none was captured
// from a real Hosting deployment. The first operator-run preview and live deploy must replace the
// reconstructed fixtures with captured ones (or prove them equal). Owner: the operator who owns the
// Firebase project. Until then an unfamiliar shape yields an *unknown* outcome, never a success.

use std::{fmt, path::Path, time::Duration};

use serde_json::Value;

use super::{hosting_config::SiteId, request::ProjectId};
use crate::{
    capture::{CaptureError, Captured},
    command::CommandSpec,
    firebase_cli::{self, link_on_host},
};

pub const LOOKUP_TIMEOUT: Duration = Duration::from_secs(90);
/// Upload, finalize and release share one bound; expiry leaves the remote outcome unknown.
pub const RELEASE_TIMEOUT: Duration = Duration::from_secs(20 * 60);

const CONSOLE_HOST: &str = "console.firebase.google.com";
const HOSTING_DOMAINS: [&str; 2] = ["web.app", "firebaseapp.com"];

/// Where a Firebase subprocess runs and what it is told, always explicitly: the project is never
/// the developer's active or default one, and the configuration is the run's effective file.
#[derive(Debug, Clone, Copy)]
pub struct Invocation<'a> {
    pub project: &'a ProjectId,
    pub config: &'a Path,
    /// The private run directory: the CLI's `firebase-debug.log` lands here, not in the checkout.
    pub cwd: &'a Path,
}

fn base(inv: &Invocation<'_>, args: &[&str]) -> CommandSpec {
    firebase_cli::command(inv.cwd)
        .args(args.iter().copied())
        .args([
            "--project".to_owned(),
            inv.project.as_str().to_owned(),
            "--config".to_owned(),
            inv.config.display().to_string(),
        ])
}

/// Read-only lookup of the project's Hosting sites. Needs no write permission.
pub fn sites_command(inv: &Invocation<'_>) -> CommandSpec {
    base(inv, &["hosting:sites:list"]).args(["--non-interactive", "--json"])
}

/// What distinguishes the two release commands.
#[derive(Debug, Clone, Copy)]
pub enum ReleaseKind<'a> {
    /// `firebase hosting:channel:deploy <channel> --only <target> ...`. `--only` takes the bare
    /// target name. `--no-authorized-domains` stops the CLI from editing Firebase Auth's
    /// authorized domains, a side effect Cantos does not want.
    Preview {
        channel: &'a str,
        target: &'a str,
        expires: Option<&'a str>,
    },
    /// `firebase deploy --only hosting:<target> ...`. The preview command refuses `live`.
    Live { target: &'a str, message: &'a str },
}

pub fn release_command(inv: &Invocation<'_>, kind: ReleaseKind<'_>) -> CommandSpec {
    match kind {
        ReleaseKind::Preview {
            channel,
            target,
            expires,
        } => {
            let mut command =
                base(inv, &["hosting:channel:deploy", channel]).args(["--only", target]);
            if let Some(expires) = expires {
                command = command.args(["--expires", expires]);
            }
            command.args(["--no-authorized-domains", "--non-interactive", "--json"])
        }
        ReleaseKind::Live { target, message } => base(inv, &["deploy"])
            .args(["--only".to_owned(), format!("hosting:{target}")])
            .args(["--message".to_owned(), message.to_owned()])
            .args(["--non-interactive", "--json"]),
    }
}

/// Why the CLI or the service refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionKind {
    Authentication,
    Authorization,
    /// The project does not exist or the account cannot see it.
    ProjectAccess,
    SiteNotFound,
    /// Both release commands first look up the project's *default* Hosting site, even when the
    /// configuration names other sites.
    NoDefaultSite,
    InvalidRequest,
}

impl fmt::Display for RejectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RejectionKind::Authentication => {
                "authentication was rejected (run `firebase login` or supply Application Default Credentials)"
            }
            RejectionKind::Authorization => {
                "the account lacks a required Hosting permission in this project"
            }
            RejectionKind::ProjectAccess => {
                "the project does not exist or the account cannot access it"
            }
            RejectionKind::SiteNotFound => "the Hosting site does not exist in this project",
            RejectionKind::NoDefaultSite => {
                "the project has no default Hosting site, which the Firebase CLI requires before it deploys (set up Hosting in the console)"
            }
            RejectionKind::InvalidRequest => "the request was rejected as invalid",
        })
    }
}

pub fn classify_rejection(text: &str) -> Option<RejectionKind> {
    let lower = text.to_ascii_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|needle| lower.contains(needle));
    if any(&[
        "firebase login",
        "failed to authenticate",
        "credentials are no longer valid",
        "unauthenticated",
        "http error: 401",
    ]) {
        Some(RejectionKind::Authentication)
    } else if any(&[
        "missing the following required permissions",
        "authorization failed",
        "http error: 403",
        "does not have permission",
        "permission denied",
    ]) {
        Some(RejectionKind::Authorization)
    } else if any(&[
        "could not determine the default site",
        "as there is no hosting site",
    ]) {
        Some(RejectionKind::NoDefaultSite)
    } else if any(&[
        "failed to get firebase project",
        "project does not exist",
        "invalid project selection",
    ]) {
        Some(RejectionKind::ProjectAccess)
    } else if any(&["http error: 404", "requested entity was not found"]) {
        Some(RejectionKind::SiteNotFound)
    } else if any(&[
        "http error: 400",
        "not configured for project",
        "not detected in firebase.json",
        "cannot deploy to the live channel",
        "flag must be a duration",
        "\"expires\" flag",
    ]) {
        Some(RejectionKind::InvalidRequest)
    } else {
        None
    }
}

fn keep_link(uri: &str) -> bool {
    if link_on_host(uri, CONSOLE_HOST).is_some() {
        return true;
    }
    let Some(authority) = uri
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
    else {
        return false;
    };
    HOSTING_DOMAINS
        .iter()
        .any(|domain| authority.ends_with(&format!(".{domain}")))
}

/// Redact child output for display or saving. Hosting URLs and the console stay: they are public
/// by design; addresses, tokens and every other link do not.
pub fn redact(text: &str) -> String {
    firebase_cli::redact(text, &[], &keep_link)
}

/// The most informative single line of a failure, redacted. Prefers the JSON `error` message.
fn detail(captured: &Captured) -> String {
    let message = json_document(&captured.stdout).and_then(|document| {
        document
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    match message {
        Some(message) => {
            let line = firebase_cli::strip_ansi(&message)
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("no diagnostic output")
                .chars()
                .take(240)
                .collect::<String>();
            redact(&line)
        }
        None => firebase_cli::error_line(&captured.stderr, &captured.stdout, &[], &keep_link),
    }
}

/// The outermost JSON object in `text`. The CLI may print warnings (for example a Node
/// deprecation notice) around the document.
fn json_document(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start < end)
        .then(|| serde_json::from_str(&text[start..=end]).ok())
        .flatten()
}

/// The facts a site lookup is allowed to keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    /// The site's default `https://<site>.web.app` URL, only when it names exactly this site.
    pub default_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DestinationError {
    /// The lookup did not complete (timeout, interruption, start failure).
    Unavailable(String),
    Rejected(RejectionKind),
    /// The CLI failed in a way that is not recognised.
    ToolFailed {
        code: i32,
        detail: String,
    },
    /// The CLI succeeded but the project has no such site (or no site at all).
    SiteNotFound,
    Unrecognized(&'static str),
}

impl fmt::Display for DestinationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DestinationError::Unavailable(why) => {
                write!(f, "site lookup did not complete: {why}")
            }
            DestinationError::Rejected(kind) => write!(f, "site lookup refused: {kind}"),
            DestinationError::ToolFailed { code, detail } => {
                write!(f, "site lookup failed (exit {code}): {detail}")
            }
            DestinationError::SiteNotFound => f.write_str(
                "the mapped Hosting site is not in this project; nothing was built or uploaded (create it with `firebase hosting:sites:create`, then fix .firebaserc)",
            ),
            DestinationError::Unrecognized(what) => {
                write!(f, "unrecognized site lookup response: {what}")
            }
        }
    }
}

pub fn interpret_sites(
    site: &SiteId,
    result: Result<Captured, CaptureError>,
) -> Result<Destination, DestinationError> {
    let captured = result.map_err(|error| DestinationError::Unavailable(error.to_string()))?;
    if captured.code != 0 {
        let text = format!("{}\n{}", captured.stdout, captured.stderr);
        return Err(match classify_rejection(&text) {
            Some(kind) => DestinationError::Rejected(kind),
            None => DestinationError::ToolFailed {
                code: captured.code,
                detail: detail(&captured),
            },
        });
    }
    let document = json_document(&captured.stdout)
        .ok_or(DestinationError::Unrecognized("no JSON document"))?;
    if document.get("status").and_then(Value::as_str) != Some("success") {
        return Err(DestinationError::Unrecognized("status is not success"));
    }
    let sites = document
        .pointer("/result/sites")
        .and_then(Value::as_array)
        .ok_or(DestinationError::Unrecognized("no site list"))?;
    let found = sites.iter().find(|entry| {
        entry
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| names_site(name, site))
    });
    let entry = found.ok_or(DestinationError::SiteNotFound)?;
    let default_url = entry
        .get("defaultUrl")
        .and_then(Value::as_str)
        .filter(|url| default_url_of(url, site))
        .map(str::to_owned);
    Ok(Destination { default_url })
}

/// `projects/<id-or-number>/sites/<site>` for exactly this site.
fn names_site(name: &str, site: &SiteId) -> bool {
    let parts: Vec<&str> = name.split('/').collect();
    matches!(parts.as_slice(), ["projects", project, "sites", id]
        if !project.is_empty() && *id == site.as_str())
}

fn default_url_of(url: &str, site: &SiteId) -> bool {
    HOSTING_DOMAINS
        .iter()
        .any(|domain| url == format!("https://{}.{domain}", site.as_str()))
}

/// What was released, read only from the CLI's own result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Released {
    /// `live`, or the preview channel ID.
    pub channel: String,
    pub site: String,
    /// The Hosting version ID, when the result named one.
    pub version: Option<String>,
    /// The URL the release is served at: the preview URL from the result, or the site's default
    /// URL from the earlier lookup for a live release.
    pub url: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownReason {
    TimedOut(Duration),
    Interrupted,
    OutputLimit,
    Plumbing(String),
    /// The CLI reported success but no recognised release, or its output changed.
    UnrecognizedOutput,
    /// A nonzero exit that is not a recognised refusal.
    UnclassifiedFailure {
        code: i32,
    },
}

impl fmt::Display for UnknownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnknownReason::TimedOut(after) => {
                write!(f, "the CLI did not finish within {}s", after.as_secs())
            }
            UnknownReason::Interrupted => f.write_str("the run was interrupted"),
            UnknownReason::OutputLimit => f.write_str("the CLI output exceeded the 4 MiB bound"),
            UnknownReason::Plumbing(why) => write!(f, "local capture failed: {why}"),
            UnknownReason::UnrecognizedOutput => f.write_str(
                "the CLI exited 0 but reported no recognised release (its output may have changed)",
            ),
            UnknownReason::UnclassifiedFailure { code } => {
                write!(f, "the CLI failed (exit {code}) in an unrecognised way")
            }
        }
    }
}

/// The result of the one remote write, distinguishing what is known from what is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteOutcome {
    /// No process ran; nothing was written.
    NotStarted(String),
    /// Exit 0 and a recognised release naming this site, target and channel.
    Confirmed { release: Released },
    /// The CLI or service refused with a recognised reason and no release was reported. A preview
    /// channel, or an uploaded version that was never released, may still exist; neither changes
    /// what the live site serves.
    Rejected { kind: RejectionKind, detail: String },
    /// The write may have happened: reconcile in the Firebase console before replaying.
    Unknown { reason: UnknownReason },
}

impl RemoteOutcome {
    pub fn release(&self) -> Option<&Released> {
        match self {
            RemoteOutcome::Confirmed { release } => Some(release),
            _ => None,
        }
    }

    /// Stable machine word for the receipt.
    pub fn state(&self) -> &'static str {
        match self {
            RemoteOutcome::NotStarted(_) => "not_started",
            RemoteOutcome::Confirmed { .. } => "confirmed",
            RemoteOutcome::Rejected { .. } => "rejected",
            RemoteOutcome::Unknown { .. } => "unknown",
        }
    }
}

/// What the interpreter must already know to accept a result as *this* release.
#[derive(Debug, Clone, Copy)]
pub struct Expected<'a> {
    pub target: &'a str,
    pub site: &'a SiteId,
    /// `live`, or the preview channel ID.
    pub channel: &'a str,
    /// The live URL from the earlier site lookup; unused for a preview.
    pub default_url: Option<&'a str>,
}

pub fn interpret_release(
    result: Result<Captured, CaptureError>,
    timeout: Duration,
    expected: &Expected<'_>,
) -> RemoteOutcome {
    match result {
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
            RemoteOutcome::Unknown { reason }
        }
        Ok(captured) => interpret_exit(&captured, expected),
    }
}

fn interpret_exit(captured: &Captured, expected: &Expected<'_>) -> RemoteOutcome {
    if captured.code == 0 {
        return match recognised_release(&captured.stdout, expected) {
            Some(release) => RemoteOutcome::Confirmed { release },
            None => RemoteOutcome::Unknown {
                reason: UnknownReason::UnrecognizedOutput,
            },
        };
    }
    // A release is reported only on success, so any release text here would be the CLI's own
    // error message; it is never read as one.
    let text = format!("{}\n{}", captured.stdout, captured.stderr);
    match classify_rejection(&text) {
        Some(kind) => RemoteOutcome::Rejected {
            kind,
            detail: detail(captured),
        },
        None => RemoteOutcome::Unknown {
            reason: UnknownReason::UnclassifiedFailure {
                code: captured.code,
            },
        },
    }
}

fn recognised_release(stdout: &str, expected: &Expected<'_>) -> Option<Released> {
    let document = json_document(stdout)?;
    if document.get("status").and_then(Value::as_str) != Some("success") {
        return None;
    }
    let result = document.get("result")?;
    if expected.channel == "live" {
        live_release(result, expected)
    } else {
        preview_release(result, expected)
    }
}

fn preview_release(result: &Value, expected: &Expected<'_>) -> Option<Released> {
    let results = result.as_object()?;
    // Exactly the selected target: a second entry would mean another site was touched.
    if results.len() != 1 {
        return None;
    }
    let entry = results.get(expected.target)?;
    if entry.get("target").and_then(Value::as_str) != Some(expected.target)
        || entry.get("site").and_then(Value::as_str) != Some(expected.site.as_str())
    {
        return None;
    }
    let url = entry.get("url").and_then(Value::as_str)?;
    if !preview_url_of(url, expected.site, expected.channel) {
        return None;
    }
    let version = match entry.get("version").and_then(Value::as_str) {
        None | Some("") => None,
        Some(id) if version_id(id) => Some(id.to_owned()),
        Some(_) => return None,
    };
    let expires_at = entry
        .get("expireTime")
        .and_then(Value::as_str)
        .filter(|time| timestamp_like(time))
        .map(str::to_owned);
    Some(Released {
        channel: expected.channel.to_owned(),
        site: expected.site.as_str().to_owned(),
        version,
        url: Some(url.to_owned()),
        expires_at,
    })
}

fn live_release(result: &Value, expected: &Expected<'_>) -> Option<Released> {
    let name = result.get("hosting")?.as_str()?;
    let version = version_of(name, expected.site)?;
    Some(Released {
        channel: "live".to_owned(),
        site: expected.site.as_str().to_owned(),
        version: Some(version),
        url: expected.default_url.map(str::to_owned),
        expires_at: None,
    })
}

/// `[projects/<x>/]sites/<site>/versions/<id>` for exactly this site.
fn version_of(name: &str, site: &SiteId) -> Option<String> {
    let parts: Vec<&str> = name.split('/').collect();
    let tail = match parts.as_slice() {
        ["projects", project, rest @ ..] if !project.is_empty() => rest,
        rest => rest,
    };
    match tail {
        ["sites", id, "versions", version] if *id == site.as_str() && version_id(version) => {
            Some((*version).to_owned())
        }
        _ => None,
    }
}

fn version_id(text: &str) -> bool {
    (1..=64).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn timestamp_like(text: &str) -> bool {
    text.len() <= 40
        && text.starts_with(|c: char| c.is_ascii_digit())
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'-' | b':' | b'.' | b'T' | b'Z' | b'+'))
}

/// Bind the reported origin to the requested site AND channel. The documented URL shape is
/// `https://<site>--<channel>-<random hash>.<hosting domain>`; unfamiliar shapes stay unknown.
/// https://firebase.google.com/docs/hosting/manage-hosting-resources
fn preview_url_of(url: &str, site: &SiteId, channel: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    // A result is an origin, not an arbitrary path or a URL carrying query/fragment data.
    let authority = rest.strip_suffix('/').unwrap_or(rest);
    let prefix = format!("{}--{channel}-", site.as_str());
    HOSTING_DOMAINS.iter().any(|domain| {
        authority
            .strip_suffix(&format!(".{domain}"))
            .and_then(|label| label.strip_prefix(&prefix))
            .is_some_and(|hash| {
                !hash.is_empty()
                    && hash
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITES: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-sites-list.json");
    const SITES_EMPTY: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-sites-list-empty.json");
    const PREVIEW: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-preview-success.json");
    const LIVE: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-live-success.json");
    const E_AUTH: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-error-auth.json");
    const E_PERMISSION: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-error-permission.json");
    const E_SITE: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-error-site-missing.json");
    const E_UPLOAD: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-error-upload-failed.json");
    const E_NO_DEFAULT: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-error-no-default-site.json");
    const CHANGED: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-preview-changed-shape.json");

    fn project() -> ProjectId {
        ProjectId::parse("cantos-example").unwrap()
    }

    fn site() -> SiteId {
        SiteId::parse("example-theatre").unwrap()
    }

    fn ok(code: i32, stdout: &str, stderr: &str) -> Result<Captured, CaptureError> {
        Ok(Captured {
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        })
    }

    fn preview_expected<'a>(site: &'a SiteId) -> Expected<'a> {
        Expected {
            target: "theatre",
            site,
            channel: "review-hosting",
            default_url: None,
        }
    }

    fn live_expected<'a>(site: &'a SiteId) -> Expected<'a> {
        Expected {
            target: "theatre",
            site,
            channel: "live",
            default_url: Some("https://example-theatre.web.app"),
        }
    }

    #[test]
    fn argv_for_every_command_is_exact_shell_free_and_names_project_and_config() {
        let cwd = Path::new("/repo/artifacts/hosting/run with space");
        let config = cwd.join("firebase.json");
        let project = project();
        let inv = Invocation {
            project: &project,
            config: &config,
            cwd,
        };
        let sites = sites_command(&inv);
        assert_eq!(sites.program, "firebase");
        assert_eq!(sites.cwd, cwd);
        assert_eq!(
            sites.args,
            [
                "hosting:sites:list",
                "--project",
                "cantos-example",
                "--config",
                "/repo/artifacts/hosting/run with space/firebase.json",
                "--non-interactive",
                "--json"
            ]
        );
        let preview = release_command(
            &inv,
            ReleaseKind::Preview {
                channel: "review-hosting",
                target: "theatre",
                expires: Some("7d"),
            },
        );
        assert_eq!(
            preview.args,
            [
                "hosting:channel:deploy",
                "review-hosting",
                "--project",
                "cantos-example",
                "--config",
                "/repo/artifacts/hosting/run with space/firebase.json",
                "--only",
                "theatre",
                "--expires",
                "7d",
                "--no-authorized-domains",
                "--non-interactive",
                "--json"
            ]
        );
        let live = release_command(
            &inv,
            ReleaseKind::Live {
                target: "studio",
                message: "cantos-studio 0123456789ab run-1",
            },
        );
        assert_eq!(
            live.args,
            [
                "deploy",
                "--project",
                "cantos-example",
                "--config",
                "/repo/artifacts/hosting/run with space/firebase.json",
                "--only",
                "hosting:studio",
                "--message",
                "cantos-studio 0123456789ab run-1",
                "--non-interactive",
                "--json"
            ]
        );
    }

    #[test]
    fn the_preview_command_omits_expires_unless_asked_and_never_names_live() {
        let cwd = Path::new("/r");
        let config = cwd.join("firebase.json");
        let project = project();
        let inv = Invocation {
            project: &project,
            config: &config,
            cwd,
        };
        let preview = release_command(
            &inv,
            ReleaseKind::Preview {
                channel: "pr-199",
                target: "theatre",
                expires: None,
            },
        );
        assert!(!preview.args.contains(&"--expires".to_owned()));
        assert!(!preview.args.contains(&"live".to_owned()));
        let live = release_command(
            &inv,
            ReleaseKind::Live {
                target: "theatre",
                message: "m",
            },
        );
        assert!(!live
            .args
            .iter()
            .any(|arg| arg.contains("channel") || arg == "--expires"));
        // Neither release command can reach Firebase Auth domains or another product.
        assert!(preview.args.contains(&"--no-authorized-domains".to_owned()));
        for command in [&preview, &live] {
            assert!(command.args.contains(&"--json".to_owned()));
            assert!(command.args.contains(&"--non-interactive".to_owned()));
            assert!(!command.args.iter().any(|a| a == "--all" || a == "--except"));
        }
    }

    #[test]
    fn a_site_lookup_succeeds_only_for_a_site_named_exactly_and_keeps_only_its_default_url() {
        let found = interpret_sites(&site(), ok(0, SITES, "")).unwrap();
        assert_eq!(
            found.default_url.as_deref(),
            Some("https://example-theatre.web.app")
        );
        let other = SiteId::parse("example-studio").unwrap();
        assert_eq!(
            interpret_sites(&other, ok(0, SITES, ""))
                .unwrap()
                .default_url
                .as_deref(),
            Some("https://example-studio.web.app")
        );
        // A prefix, a substring or another project's site is not the site.
        for near in ["example", "example-theatre-2", "theatre"] {
            let near = SiteId::parse(near).unwrap();
            assert_eq!(
                interpret_sites(&near, ok(0, SITES, "")),
                Err(DestinationError::SiteNotFound),
                "{near:?}"
            );
        }
        assert_eq!(
            interpret_sites(&site(), ok(0, SITES_EMPTY, "")),
            Err(DestinationError::SiteNotFound)
        );
    }

    #[test]
    fn a_default_url_that_does_not_name_the_site_is_not_kept() {
        let tampered = SITES.replace(
            "https://example-theatre.web.app",
            "https://elsewhere.example",
        );
        assert_eq!(
            interpret_sites(&site(), ok(0, &tampered, "")),
            Ok(Destination { default_url: None })
        );
    }

    #[test]
    fn site_lookup_failures_keep_their_category_and_never_echo_unrelated_output() {
        assert_eq!(
            interpret_sites(&site(), ok(1, E_AUTH, "")),
            Err(DestinationError::Rejected(RejectionKind::Authentication))
        );
        assert_eq!(
            interpret_sites(&site(), ok(1, E_PERMISSION, "")),
            Err(DestinationError::Rejected(RejectionKind::Authorization))
        );
        assert_eq!(
            interpret_sites(&site(), ok(1, E_SITE, "")),
            Err(DestinationError::Rejected(RejectionKind::SiteNotFound))
        );
        match interpret_sites(&site(), ok(1, E_UPLOAD, "")) {
            Err(DestinationError::ToolFailed { code: 1, detail }) => {
                assert!(detail.contains("503"), "{detail}");
            }
            other => panic!("{other:?}"),
        }
        for (stdout, expected) in [
            ("", DestinationError::Unrecognized("no JSON document")),
            (
                "{not json}",
                DestinationError::Unrecognized("no JSON document"),
            ),
            (
                r#"{"status":"error","error":"x"}"#,
                DestinationError::Unrecognized("status is not success"),
            ),
            (
                r#"{"status":"success","result":{}}"#,
                DestinationError::Unrecognized("no site list"),
            ),
        ] {
            assert_eq!(interpret_sites(&site(), ok(0, stdout, "")), Err(expected));
        }
        let timed_out = Err(CaptureError::TimedOut {
            after: LOOKUP_TIMEOUT,
            partial: Default::default(),
        });
        assert!(matches!(
            interpret_sites(&site(), timed_out),
            Err(DestinationError::Unavailable(_))
        ));
    }

    #[test]
    fn a_preview_is_confirmed_only_for_the_selected_target_site_and_a_hosting_url() {
        let site = site();
        match interpret_release(
            ok(0, PREVIEW, ""),
            RELEASE_TIMEOUT,
            &preview_expected(&site),
        ) {
            RemoteOutcome::Confirmed { release } => {
                assert_eq!(release.channel, "review-hosting");
                assert_eq!(release.site, "example-theatre");
                assert_eq!(release.version.as_deref(), Some("0123456789abcdef"));
                assert_eq!(
                    release.url.as_deref(),
                    Some("https://example-theatre--review-hosting-a1b2c3d4.web.app")
                );
                assert_eq!(
                    release.expires_at.as_deref(),
                    Some("2026-10-23T04:00:00.000000000Z")
                );
            }
            other => panic!("{other:?}"),
        }
        let other_site = SiteId::parse("example-studio").unwrap();
        for (label, outcome) in [
            (
                "another site",
                interpret_release(
                    ok(0, PREVIEW, ""),
                    RELEASE_TIMEOUT,
                    &preview_expected(&other_site),
                ),
            ),
            (
                "another target",
                interpret_release(
                    ok(0, PREVIEW, ""),
                    RELEASE_TIMEOUT,
                    &Expected {
                        target: "studio",
                        ..preview_expected(&site)
                    },
                ),
            ),
            (
                "changed output shape",
                interpret_release(
                    ok(0, CHANGED, ""),
                    RELEASE_TIMEOUT,
                    &preview_expected(&site),
                ),
            ),
            (
                "live output for a preview",
                interpret_release(ok(0, LIVE, ""), RELEASE_TIMEOUT, &preview_expected(&site)),
            ),
        ] {
            assert_eq!(
                outcome,
                RemoteOutcome::Unknown {
                    reason: UnknownReason::UnrecognizedOutput
                },
                "{label}"
            );
        }
    }

    #[test]
    fn a_preview_url_must_be_an_https_hosting_host_that_begins_with_the_site() {
        let site = site();
        for bad in [
            "http://example-theatre--x-1.web.app",
            "https://example-theatre.web.app",
            "https://example-studio--x-1.web.app",
            "https://example-theatre--x-1.evil.example",
            "https://example-theatre--x-1.web.app.evil.example",
            "https://example-theatre--.web.app",
            "https://example-theatre--X-1.web.app",
            "https://evil.example/example-theatre--x-1.web.app",
            "https://example-theatre--another-channel-a1b2c3d4.web.app",
            "https://example-theatre--review-hosting-extra-a1b2c3d4.web.app",
            "https://example-theatre--review-hosting-.web.app",
            "https://example-theatre--review-hosting-a1b2c3d4.web.app/private",
            "https://example-theatre--review-hosting-a1b2c3d4.web.app?token=example",
            "https://example-theatre--review-hosting-a1b2c3d4.web.app#other",
        ] {
            let body = PREVIEW.replace(
                "https://example-theatre--review-hosting-a1b2c3d4.web.app",
                bad,
            );
            assert_eq!(
                interpret_release(ok(0, &body, ""), RELEASE_TIMEOUT, &preview_expected(&site)),
                RemoteOutcome::Unknown {
                    reason: UnknownReason::UnrecognizedOutput
                },
                "{bad}"
            );
        }
        let firebaseapp = PREVIEW.replace(".web.app", ".firebaseapp.com");
        assert!(matches!(
            interpret_release(
                ok(0, &firebaseapp, ""),
                RELEASE_TIMEOUT,
                &preview_expected(&site)
            ),
            RemoteOutcome::Confirmed { .. }
        ));
    }

    #[test]
    fn a_second_result_entry_means_another_site_was_touched_and_is_never_a_success() {
        let site = site();
        let two = r#"{"status":"success","result":{
          "theatre":{"target":"theatre","site":"example-theatre","url":"https://example-theatre--a-1.web.app","version":"v1"},
          "studio":{"target":"studio","site":"example-studio","url":"https://example-studio--a-1.web.app","version":"v2"}}}"#;
        assert_eq!(
            interpret_release(ok(0, two, ""), RELEASE_TIMEOUT, &preview_expected(&site)),
            RemoteOutcome::Unknown {
                reason: UnknownReason::UnrecognizedOutput
            }
        );
    }

    #[test]
    fn a_live_release_is_confirmed_by_a_version_of_exactly_this_site() {
        let site = site();
        match interpret_release(ok(0, LIVE, ""), RELEASE_TIMEOUT, &live_expected(&site)) {
            RemoteOutcome::Confirmed { release } => {
                assert_eq!(release.channel, "live");
                assert_eq!(release.version.as_deref(), Some("0123456789abcdef"));
                assert_eq!(
                    release.url.as_deref(),
                    Some("https://example-theatre.web.app")
                );
                assert_eq!(release.expires_at, None);
            }
            other => panic!("{other:?}"),
        }
        let unknown = RemoteOutcome::Unknown {
            reason: UnknownReason::UnrecognizedOutput,
        };
        for body in [
            r#"{"status":"success","result":{"hosting":"projects/000000000000/sites/example-studio/versions/abc"}}"#,
            r#"{"status":"success","result":{"hosting":["projects/p/sites/example-theatre/versions/a","projects/p/sites/example-theatre/versions/b"]}}"#,
            r#"{"status":"success","result":{"hosting":"projects/p/sites/example-theatre/versions/"}}"#,
            r#"{"status":"success","result":{"hosting":"projects/p/sites/example-theatre/versions/a/b"}}"#,
            r#"{"status":"success","result":{}}"#,
            r#"{"status":"success"}"#,
            "Deploy complete!",
        ] {
            assert_eq!(
                interpret_release(ok(0, body, ""), RELEASE_TIMEOUT, &live_expected(&site)),
                unknown,
                "{body}"
            );
        }
        // The resource name may omit the project segment.
        let short =
            r#"{"status":"success","result":{"hosting":"sites/example-theatre/versions/abc"}}"#;
        assert!(matches!(
            interpret_release(ok(0, short, ""), RELEASE_TIMEOUT, &live_expected(&site)),
            RemoteOutcome::Confirmed { .. }
        ));
    }

    #[test]
    fn a_live_release_never_invents_a_url_the_lookup_did_not_observe() {
        let site = site();
        let expected = Expected {
            default_url: None,
            ..live_expected(&site)
        };
        match interpret_release(ok(0, LIVE, ""), RELEASE_TIMEOUT, &expected) {
            RemoteOutcome::Confirmed { release } => assert_eq!(release.url, None),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn recognised_refusals_keep_their_category_and_the_unrecognised_stay_unknown() {
        let site = site();
        for (stdout, kind) in [
            (E_AUTH, RejectionKind::Authentication),
            (E_PERMISSION, RejectionKind::Authorization),
            (E_SITE, RejectionKind::SiteNotFound),
            (E_NO_DEFAULT, RejectionKind::NoDefaultSite),
        ] {
            match interpret_release(ok(1, stdout, ""), RELEASE_TIMEOUT, &preview_expected(&site)) {
                RemoteOutcome::Rejected {
                    kind: found,
                    detail,
                } => {
                    assert_eq!(found, kind);
                    assert!(!detail.is_empty());
                }
                other => panic!("{other:?}"),
            }
        }
        // A 503 mid-upload cannot say whether the channel or a version already exists.
        assert_eq!(
            interpret_release(
                ok(1, E_UPLOAD, ""),
                RELEASE_TIMEOUT,
                &preview_expected(&site)
            ),
            RemoteOutcome::Unknown {
                reason: UnknownReason::UnclassifiedFailure { code: 1 }
            }
        );
    }

    #[test]
    fn a_success_document_on_a_failing_exit_is_never_read_as_a_release() {
        let site = site();
        assert_eq!(
            interpret_release(
                ok(1, PREVIEW, ""),
                RELEASE_TIMEOUT,
                &preview_expected(&site)
            ),
            RemoteOutcome::Unknown {
                reason: UnknownReason::UnclassifiedFailure { code: 1 }
            }
        );
    }

    #[test]
    fn warnings_around_the_json_document_do_not_hide_a_release() {
        let site = site();
        let noisy = format!("(node:1) [DEP0040] DeprecationWarning: punycode\n{PREVIEW}\n");
        assert!(matches!(
            interpret_release(
                ok(0, &noisy, "warn"),
                RELEASE_TIMEOUT,
                &preview_expected(&site)
            ),
            RemoteOutcome::Confirmed { .. }
        ));
    }

    #[test]
    fn timeout_interruption_lost_output_and_plumbing_are_unknown_and_a_spawn_failure_is_not() {
        let site = site();
        let expected = preview_expected(&site);
        let partial = crate::capture::Partial {
            stdout: PREVIEW.into(),
            stderr: String::new(),
        };
        for (error, reason) in [
            (
                CaptureError::TimedOut {
                    after: RELEASE_TIMEOUT,
                    partial: partial.clone(),
                },
                UnknownReason::TimedOut(RELEASE_TIMEOUT),
            ),
            (
                CaptureError::Interrupted {
                    partial: partial.clone(),
                    started: true,
                },
                UnknownReason::Interrupted,
            ),
            (
                CaptureError::OutputLimit {
                    partial: partial.clone(),
                },
                UnknownReason::OutputLimit,
            ),
            (
                CaptureError::Plumbing {
                    message: "pipe".into(),
                    started: true,
                },
                UnknownReason::Plumbing("pipe".into()),
            ),
        ] {
            // Even when the partial output already shows a release, an interrupted write is not a
            // confirmed one: the receipt says unknown and the operator reconciles.
            assert_eq!(
                interpret_release(Err(error), RELEASE_TIMEOUT, &expected),
                RemoteOutcome::Unknown { reason }
            );
        }
        for error in [
            CaptureError::Spawn {
                program: "firebase".into(),
                message: "not found".into(),
            },
            CaptureError::Interrupted {
                partial: Default::default(),
                started: false,
            },
            CaptureError::Plumbing {
                message: "tmp".into(),
                started: false,
            },
        ] {
            assert!(matches!(
                interpret_release(Err(error), RELEASE_TIMEOUT, &expected),
                RemoteOutcome::NotStarted(_)
            ));
        }
    }

    #[test]
    fn outcomes_expose_a_stable_state_word_and_only_a_confirmed_one_has_a_release() {
        let site = site();
        let confirmed = interpret_release(
            ok(0, PREVIEW, ""),
            RELEASE_TIMEOUT,
            &preview_expected(&site),
        );
        assert_eq!(confirmed.state(), "confirmed");
        assert!(confirmed.release().is_some());
        for (outcome, state) in [
            (RemoteOutcome::NotStarted("x".into()), "not_started"),
            (
                RemoteOutcome::Rejected {
                    kind: RejectionKind::Authentication,
                    detail: String::new(),
                },
                "rejected",
            ),
            (
                RemoteOutcome::Unknown {
                    reason: UnknownReason::Interrupted,
                },
                "unknown",
            ),
        ] {
            assert_eq!(outcome.state(), state);
            assert!(outcome.release().is_none());
        }
    }

    #[test]
    fn redaction_keeps_hosting_and_console_links_and_drops_addresses_tokens_and_other_links() {
        let raw = "preview https://example-theatre--a-1.web.app and https://console.firebase.google.com/project/p/hosting \
                   see https://evil.example/x?token=abc for qa@example.com ya29.token AIzaSyFAKE";
        let clean = redact(raw);
        assert!(clean.contains("https://example-theatre--a-1.web.app"));
        assert!(clean.contains("https://console.firebase.google.com/project/p/hosting"));
        assert!(!clean.contains("evil.example"));
        assert!(!clean.contains("qa@example.com"));
        assert!(!clean.contains("ya29."));
        assert!(!clean.contains("AIza"));
        assert!(redact("https://web.app.evil.example/").contains("[url redacted]"));
    }

    #[test]
    fn failure_detail_prefers_the_json_error_message_and_is_bounded_and_redacted() {
        let long = "x".repeat(1000);
        let body = format!(
            r#"{{"status":"error","error":"HTTP Error: 403, caller qa.person@example.com lacks permission {long}"}}"#
        );
        let site = site();
        match interpret_release(ok(1, &body, ""), RELEASE_TIMEOUT, &preview_expected(&site)) {
            RemoteOutcome::Rejected { detail, .. } => {
                assert!(detail.contains("[email redacted]"), "{detail}");
                assert!(!detail.contains("qa.person"), "{detail}");
                assert!(detail.len() <= 260, "{}", detail.len());
            }
            other => panic!("{other:?}"),
        }
    }
}
