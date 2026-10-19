//! The Firebase CLI adapter, as pure functions over captured output. It names the CLI contract
//! it was written against, builds argv without a shell, checks the destination and translates
//! the process result into a closed remote-outcome vocabulary.
//!
//! The contract is `firebase-tools` **15.x, tested against 15.34.0**:
//! - `appdistribution:distribute` performs binary upload, then release-notes update, then
//!   distribution. It prints the release identity and links *after the upload*, before the later
//!   stages run, so a printed release link is never distribution success. Only exit status 0
//!   plus a recognised release establishes acceptance.
//! - `apps:sdkconfig` exits 0 with an empty `client` list for an app that does not exist, so the
//!   response itself must name the requested app and package.
//!
//! The interpreter is fail-closed: anything it does not recognise after the write may have
//! started is an *unknown* remote outcome, never a success or a clean failure.

use std::{fmt, path::Path, time::Duration};

use serde_json::Value;

use super::request::AppId;
pub use crate::firebase_cli::{
    parse_version, strip_ansi, version_command, version_supported, MINIMUM_VERSION,
    SUPPORTED_MAJOR, TESTED_VERSION, VERSION_TIMEOUT,
};
use crate::{
    capture::{CaptureError, Captured, Partial},
    command::CommandSpec,
    firebase_cli::{self, link_on_host as allowed_link},
};

pub const LOOKUP_TIMEOUT: Duration = Duration::from_secs(90);
/// Upload, notes and distribution share one bound; expiry leaves the remote outcome unknown.
pub const DISTRIBUTE_TIMEOUT: Duration = Duration::from_secs(20 * 60);

const CONSOLE_HOST: &str = "console.firebase.google.com";
const TESTING_HOST: &str = "appdistribution.firebase.google.com";

fn firebase(cwd: &Path) -> CommandSpec {
    firebase_cli::command(cwd)
}

/// Only the durable console and tester links survive redaction of App Distribution output.
fn keep_distribution_link(uri: &str) -> bool {
    allowed_link(uri, CONSOLE_HOST).is_some() || allowed_link(uri, TESTING_HOST).is_some()
}

/// Read-only registration lookup. Its stdout is a Google Services config including an API key:
/// callers parse it in memory and never print or store it.
pub fn lookup_command(app: &AppId, cwd: &Path) -> CommandSpec {
    firebase(cwd).args([
        "apps:sdkconfig",
        "ANDROID",
        app.as_str(),
        "--non-interactive",
    ])
}

/// Everything the distribute invocation needs. Tester addresses travel only inside
/// `testers_file`, never in argv.
pub struct DistributeInputs<'a> {
    pub run_dir: &'a Path,
    pub staged_apk: &'a Path,
    pub app_id: &'a AppId,
    pub groups: &'a [String],
    pub notes_file: &'a Path,
    pub testers_file: Option<&'a Path>,
}

pub fn distribute_command(inputs: &DistributeInputs<'_>) -> CommandSpec {
    // `--flag=value` keeps a value that begins with `-` from being read as another option, and
    // a notes file preserves multiline/Unicode text that an argv value would have to escape.
    let mut command = firebase(inputs.run_dir).args([
        "appdistribution:distribute".to_owned(),
        inputs.staged_apk.to_string_lossy().into_owned(),
        format!("--app={}", inputs.app_id.as_str()),
        format!("--release-notes-file={}", inputs.notes_file.display()),
    ]);
    if !inputs.groups.is_empty() {
        command = command.args([format!("--groups={}", inputs.groups.join(","))]);
    }
    if let Some(file) = inputs.testers_file {
        command = command.args([format!("--testers-file={}", file.display())]);
    }
    command.args(["--non-interactive"])
}

/// Why a service or CLI refused before any write was accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionKind {
    Authentication,
    Authorization,
    AppNotFound,
    InvalidRequest,
}

impl fmt::Display for RejectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RejectionKind::Authentication => {
                "authentication was rejected (run `firebase login` or supply Application Default Credentials)"
            }
            RejectionKind::Authorization => "the account lacks permission for this app",
            RejectionKind::AppNotFound => {
                "the app is not onboarded to App Distribution (press Get started in the Firebase console)"
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
        "requires authentication",
        "invalid_grant",
        "unauthenticated",
        "http error: 401",
    ]) {
        Some(RejectionKind::Authentication)
    } else if any(&[
        "http error: 403",
        "does not have permission",
        "permission denied",
    ]) {
        Some(RejectionKind::Authorization)
    } else if any(&[
        "could not find your app",
        "http error: 404",
        "requested entity was not found",
    ]) {
        Some(RejectionKind::AppNotFound)
    } else if any(&[
        "invalid firebase app id",
        "unsupported file format",
        "does not exist: verify",
        "http error: 400",
    ]) {
        Some(RejectionKind::InvalidRequest)
    } else {
        None
    }
}

/// The destination facts a lookup is allowed to keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    pub package: String,
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
    /// The CLI succeeded but no Android app with this ID is registered.
    NotRegistered,
    /// The response named different app(s) than the one requested.
    AppIdMismatch,
    PackageMismatch {
        expected: String,
        found: String,
    },
    Unrecognized(&'static str),
}

impl fmt::Display for DestinationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DestinationError::Unavailable(why) => write!(f, "destination lookup did not complete: {why}"),
            DestinationError::Rejected(kind) => write!(f, "destination lookup refused: {kind}"),
            DestinationError::ToolFailed { code, detail } => {
                write!(f, "destination lookup failed (exit {code}): {detail}")
            }
            DestinationError::NotRegistered => f.write_str(
                "no Android app with this ID is registered in the project the ID names; nothing was built or uploaded",
            ),
            DestinationError::AppIdMismatch => {
                f.write_str("the lookup response does not name the requested app")
            }
            DestinationError::PackageMismatch { expected, found } => write!(
                f,
                "the Firebase app is registered for package `{found}`, the selected channel requires `{expected}`"
            ),
            DestinationError::Unrecognized(what) => write!(f, "unrecognized lookup response: {what}"),
        }
    }
}

#[cfg(test)]
pub fn interpret_lookup(
    requested: &AppId,
    result: Result<Captured, CaptureError>,
) -> Result<Registered, DestinationError> {
    interpret_lookup_package(requested, super::request::DEVELOPMENT_PACKAGE, result)
}

pub fn interpret_lookup_package(
    requested: &AppId,
    expected_package: &str,
    result: Result<Captured, CaptureError>,
) -> Result<Registered, DestinationError> {
    let captured = result.map_err(|error| DestinationError::Unavailable(error.to_string()))?;
    if captured.code != 0 {
        let text = format!("{}\n{}", captured.stdout, captured.stderr);
        return Err(match classify_rejection(&text) {
            Some(kind) => DestinationError::Rejected(kind),
            None => DestinationError::ToolFailed {
                code: captured.code,
                detail: error_line(&captured.stderr, &captured.stdout, &[]),
            },
        });
    }
    // The body is a Google Services config; parse only the registration facts from it.
    let body = {
        let start = captured.stdout.find('{');
        let end = captured.stdout.rfind('}');
        match (start, end) {
            (Some(start), Some(end)) if start < end => &captured.stdout[start..=end],
            _ => return Err(DestinationError::Unrecognized("no JSON document")),
        }
    };
    let document: Value = serde_json::from_str(body)
        .map_err(|_| DestinationError::Unrecognized("the response is not valid JSON"))?;
    let clients = document
        .get("client")
        .and_then(Value::as_array)
        .ok_or(DestinationError::Unrecognized("no client list"))?;
    if clients.is_empty() {
        return Err(DestinationError::NotRegistered);
    }
    let matching = clients.iter().find(|client| {
        client
            .pointer("/client_info/mobilesdk_app_id")
            .and_then(Value::as_str)
            == Some(requested.as_str())
    });
    let client = matching.ok_or(DestinationError::AppIdMismatch)?;
    let package = client
        .pointer("/client_info/android_client_info/package_name")
        .and_then(Value::as_str)
        .filter(|package| !package.is_empty())
        .ok_or(DestinationError::Unrecognized("no registered package name"))?;
    if package != expected_package {
        return Err(DestinationError::PackageMismatch {
            expected: expected_package.to_owned(),
            found: package.to_owned(),
        });
    }
    Ok(Registered {
        package: package.to_owned(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadResult {
    Created,
    Updated,
    Unmodified,
    Unspecified,
}

impl UploadResult {
    pub fn label(self) -> &'static str {
        match self {
            UploadResult::Created => "created",
            UploadResult::Updated => "updated",
            UploadResult::Unmodified => "unmodified",
            UploadResult::Unspecified => "unspecified",
        }
    }
}

/// A release the CLI named. The signed binary download link is deliberately not representable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub display_version: String,
    pub build_version: String,
    pub upload: UploadResult,
    pub console_uri: Option<String>,
    pub testing_uri: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailedStage {
    ReleaseNotes,
    Distribution,
    /// Something failed after the upload and the CLI did not say which stage.
    AfterUpload,
}

impl fmt::Display for FailedStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FailedStage::ReleaseNotes => "release-notes update",
            FailedStage::Distribution => "distribution to testers/groups",
            FailedStage::AfterUpload => "an unspecified stage after the upload",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownReason {
    TimedOut(Duration),
    Interrupted,
    OutputLimit,
    Plumbing(String),
    /// The CLI reported success but no recognised release, or its output changed.
    UnrecognizedOutput,
    /// A nonzero exit that is neither a recognised refusal nor a recognised partial success.
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
    /// Overall success: the service accepted upload, notes and distribution.
    Confirmed { release: Release },
    /// Refused before any write was accepted.
    Rejected { kind: RejectionKind, detail: String },
    /// The release exists, but a later stage failed. Testers may or may not have been notified.
    Partial {
        release: Release,
        failed: FailedStage,
        detail: String,
    },
    /// The write may have happened. Reconcile in the Firebase console before replaying.
    Unknown {
        release: Option<Release>,
        reason: UnknownReason,
    },
}

impl RemoteOutcome {
    pub fn release(&self) -> Option<&Release> {
        match self {
            RemoteOutcome::Confirmed { release } | RemoteOutcome::Partial { release, .. } => {
                Some(release)
            }
            RemoteOutcome::Unknown { release, .. } => release.as_ref(),
            _ => None,
        }
    }

    /// Stable machine word for the receipt.
    pub fn state(&self) -> &'static str {
        match self {
            RemoteOutcome::NotStarted(_) => "not_started",
            RemoteOutcome::Confirmed { .. } => "confirmed",
            RemoteOutcome::Rejected { .. } => "rejected",
            RemoteOutcome::Partial { .. } => "partial",
            RemoteOutcome::Unknown { .. } => "unknown",
        }
    }
}

/// Recognise the release lines of the pinned CLI contract in already-clean text.
fn parse_release(text: &str) -> Option<Release> {
    const UPLOADS: [(&str, UploadResult); 4] = [
        (
            "re-uploaded already existing release ",
            UploadResult::Unmodified,
        ),
        (
            "uploaded update to existing release ",
            UploadResult::Updated,
        ),
        ("uploaded new release ", UploadResult::Created),
        ("uploaded release ", UploadResult::Unspecified),
    ];
    let mut identity = None;
    let mut console_uri = None;
    let mut testing_uri = None;
    for line in text.lines() {
        let line = line.trim();
        if identity.is_none() {
            for (prefix, upload) in UPLOADS {
                let Some(position) = line.find(prefix) else {
                    continue;
                };
                let rest = line[position + prefix.len()..].strip_suffix(" successfully!");
                if let Some((display, build)) = rest.and_then(split_version) {
                    identity = Some((display, build, upload));
                }
                break;
            }
        }
        if let Some(uri) = line
            .split("View this release in the Firebase console: ")
            .nth(1)
        {
            console_uri = console_uri.or_else(|| allowed_link(uri.trim(), CONSOLE_HOST));
        }
        if let Some(uri) = line
            .split("Share this release with testers who have access: ")
            .nth(1)
        {
            testing_uri = testing_uri.or_else(|| allowed_link(uri.trim(), TESTING_HOST));
        }
    }
    identity.map(|(display_version, build_version, upload)| Release {
        display_version,
        build_version,
        upload,
        console_uri,
        testing_uri,
    })
}

/// `0.1.0-dev (1)` -> (`0.1.0-dev`, `1`).
fn split_version(rest: &str) -> Option<(String, String)> {
    let rest = rest.strip_suffix(')')?;
    let split = rest.rfind(" (")?;
    let (display, build) = (&rest[..split], &rest[split + 2..]);
    (!display.is_empty() && !build.is_empty()).then(|| (display.to_owned(), build.to_owned()))
}

/// The most informative single error line, redacted. Prefers the CLI's `Error:` line.
fn error_line(primary: &str, secondary: &str, secrets: &[&str]) -> String {
    firebase_cli::error_line(primary, secondary, secrets, &keep_distribution_link)
}

pub fn interpret_distribution(
    result: Result<Captured, CaptureError>,
    timeout: Duration,
    secrets: &[&str],
) -> RemoteOutcome {
    match result {
        Err(error) => {
            let release = error
                .partial()
                .and_then(|partial| parse_release(&combined(partial)));
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
            RemoteOutcome::Unknown { release, reason }
        }
        Ok(captured) => interpret_exit(&captured, secrets),
    }
}

fn combined(partial: &Partial) -> String {
    strip_ansi(&format!("{}\n{}", partial.stdout, partial.stderr))
}

fn interpret_exit(captured: &Captured, secrets: &[&str]) -> RemoteOutcome {
    let text = strip_ansi(&format!("{}\n{}", captured.stdout, captured.stderr));
    let release = parse_release(&text);
    if captured.code == 0 {
        return match release {
            Some(release) => RemoteOutcome::Confirmed { release },
            None => RemoteOutcome::Unknown {
                release: None,
                reason: UnknownReason::UnrecognizedOutput,
            },
        };
    }
    let lower = text.to_ascii_lowercase();
    let detail = error_line(&captured.stderr, &captured.stdout, secrets);
    match release {
        Some(release) => {
            let failed = if lower.contains("failed to update release notes") {
                FailedStage::ReleaseNotes
            } else if lower.contains("failed to distribute to testers/groups") {
                FailedStage::Distribution
            } else {
                FailedStage::AfterUpload
            };
            RemoteOutcome::Partial {
                release,
                failed,
                detail,
            }
        }
        None => match classify_rejection(&text) {
            Some(kind) => RemoteOutcome::Rejected { kind, detail },
            None => RemoteOutcome::Unknown {
                release: None,
                reason: UnknownReason::UnclassifiedFailure {
                    code: captured.code,
                },
            },
        },
    }
}

/// Make child output safe to display or save: no tester addresses, signed links, tokens or
/// credentials. Only the durable console/tester links survive. `secrets` are exact values the
/// caller knows to be sensitive.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    firebase_cli::redact(text, secrets, &keep_distribution_link)
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "1:000000000000:android:0123456789abcdef";
    const CREATED: &str = include_str!(
        "../../fixtures/firebase-tools-15.34.0/distribute-created-development-policy.txt"
    );
    const FAIL_NOTES: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/distribute-notes-failed.txt");
    const FAIL_DISTRIBUTE: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/distribute-distribution-failed.txt");
    const FAIL_AUTH: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/distribute-auth-failed.txt");
    const FAIL_APP: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/distribute-app-missing.txt");
    const SDK_EMPTY: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/sdkconfig-unregistered.json");
    const SDK_REGISTERED: &str = include_str!(
        "../../fixtures/firebase-tools-15.34.0/sdkconfig-registered-development-policy.json"
    );

    fn app() -> AppId {
        AppId::parse(APP).unwrap()
    }

    fn ok(code: i32, stdout: &str, stderr: &str) -> Result<Captured, CaptureError> {
        Ok(Captured {
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        })
    }

    #[test]
    fn distribute_argv_is_exact_shell_free_and_keeps_addresses_out_of_it() {
        let run = Path::new("/repo/artifacts/distributions/run with space");
        let apk = run.join("artifact/ứng dụng.apk");
        let notes = run.join("release-notes.txt");
        let testers = run.join("testers.txt");
        let groups = vec!["cantos-internal".to_owned(), "release".to_owned()];
        let command = distribute_command(&DistributeInputs {
            run_dir: run,
            staged_apk: &apk,
            app_id: &app(),
            groups: &groups,
            notes_file: &notes,
            testers_file: Some(&testers),
        });
        assert_eq!(command.program, "firebase");
        assert_eq!(command.cwd, run);
        assert_eq!(
            command.args,
            [
                "appdistribution:distribute".to_owned(),
                apk.to_string_lossy().into_owned(),
                format!("--app={APP}"),
                format!("--release-notes-file={}", notes.display()),
                "--groups=cantos-internal,release".to_owned(),
                format!("--testers-file={}", testers.display()),
                "--non-interactive".to_owned(),
            ]
        );
        assert!(command.env.is_empty());
        let no_testers = distribute_command(&DistributeInputs {
            run_dir: run,
            staged_apk: &apk,
            app_id: &app(),
            groups: &[],
            notes_file: &notes,
            testers_file: None,
        });
        assert!(!no_testers
            .args
            .iter()
            .any(|a| a.starts_with("--groups") || a.starts_with("--testers")));
    }

    #[test]
    fn production_destination_requires_exact_registered_production_package() {
        let body = SDK_REGISTERED.replace("com.cantos.theatre.dev", "com.cantos.theatre");
        assert_eq!(
            interpret_lookup_package(&app(), "com.cantos.theatre", ok(0, &body, ""))
                .unwrap()
                .package,
            "com.cantos.theatre"
        );
        assert!(matches!(
            interpret_lookup_package(&app(), "com.cantos.theatre", ok(0, SDK_REGISTERED, "")),
            Err(DestinationError::PackageMismatch { .. })
        ));
    }

    #[test]
    fn lookup_succeeds_only_for_the_requested_app_and_the_demo_package() {
        let registered = interpret_lookup(
            &app(),
            ok(0, SDK_REGISTERED, "- Downloading\n✔ Downloading"),
        )
        .unwrap();
        assert_eq!(registered.package, "com.cantos.theatre.dev");
    }

    #[test]
    fn an_unregistered_app_is_not_a_success_even_though_the_cli_exits_zero() {
        // Captured from a real `apps:sdkconfig` of a nonexistent app ID: exit 0, empty clients.
        assert_eq!(
            interpret_lookup(&app(), ok(0, SDK_EMPTY, "")),
            Err(DestinationError::NotRegistered)
        );
    }

    #[test]
    fn lookup_rejects_a_different_app_a_different_package_and_unreadable_responses() {
        let other = AppId::parse("1:000000000000:android:fedcba9876543210").unwrap();
        assert_eq!(
            interpret_lookup(&other, ok(0, SDK_REGISTERED, "")),
            Err(DestinationError::AppIdMismatch)
        );
        let production = SDK_REGISTERED.replace("com.cantos.theatre.dev", "com.cantos.theatre");
        assert_eq!(
            interpret_lookup(&app(), ok(0, &production, "")),
            Err(DestinationError::PackageMismatch {
                expected: "com.cantos.theatre.dev".into(),
                found: "com.cantos.theatre".into()
            })
        );
        for body in [
            "",
            "no json at all",
            "{\"client\": 3}",
            "{\"project_info\": {}}",
        ] {
            assert!(
                matches!(
                    interpret_lookup(&app(), ok(0, body, "")),
                    Err(DestinationError::Unrecognized(_))
                ),
                "{body}"
            );
        }
        let no_package = SDK_REGISTERED.replace("com.cantos.theatre.dev", "");
        assert!(matches!(
            interpret_lookup(&app(), ok(0, &no_package, "")),
            Err(DestinationError::Unrecognized(_))
        ));
    }

    #[test]
    fn lookup_failures_keep_their_category_and_never_echo_the_response() {
        assert_eq!(
            interpret_lookup(&app(), ok(1, "", FAIL_AUTH)),
            Err(DestinationError::Rejected(RejectionKind::Authentication))
        );
        assert_eq!(
            interpret_lookup(
                &app(),
                ok(
                    1,
                    "",
                    "Error: HTTP Error: 403, The caller does not have permission"
                )
            ),
            Err(DestinationError::Rejected(RejectionKind::Authorization))
        );
        match interpret_lookup(&app(), ok(1, "", "Error: something odd happened")) {
            Err(DestinationError::ToolFailed { code: 1, detail }) => {
                assert!(detail.contains("something odd"));
            }
            other => panic!("{other:?}"),
        }
        let timed_out = Err(CaptureError::TimedOut {
            after: LOOKUP_TIMEOUT,
            partial: Partial::default(),
        });
        assert!(matches!(
            interpret_lookup(&app(), timed_out),
            Err(DestinationError::Unavailable(_))
        ));
        // A key inside a successful body is never part of any error text.
        let leaky = SDK_REGISTERED.replace("com.cantos.theatre.dev", "");
        let error = interpret_lookup(&app(), ok(0, &leaky, ""))
            .unwrap_err()
            .to_string();
        assert!(!error.contains("SYNTHETIC"), "{error}");
    }

    #[test]
    fn confirmed_success_needs_exit_zero_and_a_recognised_release() {
        let outcome = interpret_distribution(ok(0, CREATED, ""), DISTRIBUTE_TIMEOUT, &[]);
        match outcome {
            RemoteOutcome::Confirmed { release } => {
                assert_eq!(release.display_version, "0.1.0-dev");
                assert_eq!(release.build_version, "1");
                assert_eq!(release.upload, UploadResult::Created);
                assert!(release
                    .console_uri
                    .unwrap()
                    .starts_with("https://console.firebase.google.com/"));
                assert!(release
                    .testing_uri
                    .unwrap()
                    .starts_with("https://appdistribution.firebase.google.com/"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn upload_variants_are_distinguished_and_reuse_is_not_exactly_once() {
        for (line, expected) in [
            (
                "uploaded new release 1.0 (7) successfully!",
                UploadResult::Created,
            ),
            (
                "uploaded update to existing release 1.0 (7) successfully!",
                UploadResult::Updated,
            ),
            (
                "re-uploaded already existing release 1.0 (7) successfully!",
                UploadResult::Unmodified,
            ),
            (
                "uploaded release 1.0 (7) successfully!",
                UploadResult::Unspecified,
            ),
        ] {
            let outcome =
                interpret_distribution(ok(0, &format!("✔  {line}\n"), ""), DISTRIBUTE_TIMEOUT, &[]);
            match outcome {
                RemoteOutcome::Confirmed { release } => {
                    assert_eq!(release.upload, expected, "{line}")
                }
                other => panic!("{line}: {other:?}"),
            }
        }
    }

    #[test]
    fn a_release_link_alone_is_not_distribution_success() {
        // The CLI prints the release links after the upload, before notes and distribution run.
        let outcome =
            interpret_distribution(ok(1, CREATED, FAIL_DISTRIBUTE), DISTRIBUTE_TIMEOUT, &[]);
        match outcome {
            RemoteOutcome::Partial {
                release,
                failed,
                detail,
            } => {
                assert_eq!(failed, FailedStage::Distribution);
                assert_eq!(release.build_version, "1");
                assert!(detail.contains("failed to distribute"));
            }
            other => panic!("{other:?}"),
        }
        let notes = interpret_distribution(ok(1, CREATED, FAIL_NOTES), DISTRIBUTE_TIMEOUT, &[]);
        assert!(matches!(
            notes,
            RemoteOutcome::Partial {
                failed: FailedStage::ReleaseNotes,
                ..
            }
        ));
        let unspecified =
            interpret_distribution(ok(1, CREATED, "Error: boom"), DISTRIBUTE_TIMEOUT, &[]);
        assert!(matches!(
            unspecified,
            RemoteOutcome::Partial {
                failed: FailedStage::AfterUpload,
                ..
            }
        ));
    }

    #[test]
    fn refusals_before_any_upload_are_confirmed_rejections() {
        let auth = interpret_distribution(ok(1, "", FAIL_AUTH), DISTRIBUTE_TIMEOUT, &[]);
        assert!(matches!(
            auth,
            RemoteOutcome::Rejected {
                kind: RejectionKind::Authentication,
                ..
            }
        ));
        let missing = interpret_distribution(ok(1, "", FAIL_APP), DISTRIBUTE_TIMEOUT, &[]);
        assert!(matches!(
            missing,
            RemoteOutcome::Rejected {
                kind: RejectionKind::AppNotFound,
                ..
            }
        ));
    }

    #[test]
    fn changed_or_unrecognised_output_preserves_uncertainty() {
        let changed = interpret_distribution(
            ok(
                0,
                "Release published OK. Link: https://example.invalid/r/1\n",
                "",
            ),
            DISTRIBUTE_TIMEOUT,
            &[],
        );
        assert_eq!(
            changed,
            RemoteOutcome::Unknown {
                release: None,
                reason: UnknownReason::UnrecognizedOutput
            }
        );
        let silent = interpret_distribution(ok(0, "", ""), DISTRIBUTE_TIMEOUT, &[]);
        assert!(matches!(
            silent,
            RemoteOutcome::Unknown {
                reason: UnknownReason::UnrecognizedOutput,
                ..
            }
        ));
        // "Failed to upload release" can hide an in-flight upload, so it is not a clean rejection.
        let lost = interpret_distribution(
            ok(1, "", "Error: Failed to upload release. network timeout"),
            DISTRIBUTE_TIMEOUT,
            &[],
        );
        assert_eq!(
            lost,
            RemoteOutcome::Unknown {
                release: None,
                reason: UnknownReason::UnclassifiedFailure { code: 1 }
            }
        );
    }

    #[test]
    fn timeout_interruption_and_lost_output_are_unknown_and_keep_a_known_release() {
        let partial = Partial {
            stdout: CREATED.into(),
            stderr: String::new(),
        };
        let timed_out = interpret_distribution(
            Err(CaptureError::TimedOut {
                after: DISTRIBUTE_TIMEOUT,
                partial: partial.clone(),
            }),
            DISTRIBUTE_TIMEOUT,
            &[],
        );
        match timed_out {
            RemoteOutcome::Unknown {
                release: Some(release),
                reason: UnknownReason::TimedOut(_),
            } => {
                assert_eq!(release.build_version, "1");
            }
            other => panic!("{other:?}"),
        }
        let interrupted = interpret_distribution(
            Err(CaptureError::Interrupted {
                partial: Partial::default(),
                started: true,
            }),
            DISTRIBUTE_TIMEOUT,
            &[],
        );
        assert!(matches!(
            interrupted,
            RemoteOutcome::Unknown {
                release: None,
                reason: UnknownReason::Interrupted
            }
        ));
        let over = interpret_distribution(
            Err(CaptureError::OutputLimit { partial }),
            DISTRIBUTE_TIMEOUT,
            &[],
        );
        assert!(matches!(
            over,
            RemoteOutcome::Unknown {
                reason: UnknownReason::OutputLimit,
                ..
            }
        ));
    }

    #[test]
    fn a_process_that_never_started_wrote_nothing() {
        for error in [
            CaptureError::Spawn {
                program: "firebase".into(),
                message: "not found".into(),
            },
            CaptureError::Interrupted {
                partial: Partial::default(),
                started: false,
            },
            CaptureError::Plumbing {
                message: "tmp".into(),
                started: false,
            },
        ] {
            assert!(matches!(
                interpret_distribution(Err(error), DISTRIBUTE_TIMEOUT, &[]),
                RemoteOutcome::NotStarted(_)
            ));
        }
        assert!(matches!(
            interpret_distribution(
                Err(CaptureError::Plumbing {
                    message: "wait failed".into(),
                    started: true
                }),
                DISTRIBUTE_TIMEOUT,
                &[]
            ),
            RemoteOutcome::Unknown {
                reason: UnknownReason::Plumbing(_),
                ..
            }
        ));
    }

    #[test]
    fn redaction_removes_addresses_signed_links_and_tokens_but_keeps_durable_links() {
        let raw = "✔  Download the release binary (link expires in 1 hour): https://firebaseappdistribution.googleapis.com/app-binaries/x?token=abc\n\
                   \u{1b}[32m✔\u{1b}[0m  View this release in the Firebase console: https://console.firebase.google.com/project/p/appdistribution/app/a/releases/1\n\
                   sent to qa.person@example.com and (other@example.org), ya29.A0ARrdaM-token AIzaSyFAKE key=abc\n\
                   Share this release with testers who have access: https://appdistribution.firebase.google.com/testerapps/a/releases/1?utm_source=firebase-tools";
        let clean = redact(raw, &["private-group"]);
        assert!(!clean.contains("qa.person"), "{clean}");
        assert!(!clean.contains("other@"), "{clean}");
        assert!(!clean.contains("token=abc"), "{clean}");
        assert!(!clean.contains("ya29."), "{clean}");
        assert!(!clean.contains("AIza"), "{clean}");
        assert!(!clean.contains("googleapis.com"), "{clean}");
        assert!(!clean.contains('\u{1b}'));
        assert!(clean.contains(
            "https://console.firebase.google.com/project/p/appdistribution/app/a/releases/1"
        ));
        assert!(clean.contains("https://appdistribution.firebase.google.com/testerapps/a/releases/1?utm_source=firebase-tools"));
        assert_eq!(
            redact("group private-group ok", &["private-group"]),
            "group [redacted] ok"
        );
    }

    #[test]
    fn the_signed_download_link_is_not_representable_in_a_release() {
        let outcome = interpret_distribution(ok(0, CREATED, ""), DISTRIBUTE_TIMEOUT, &[]);
        let debug = format!("{outcome:?}");
        assert!(!debug.contains("googleapis"), "{debug}");
        assert!(!debug.contains("SYNTHETIC"), "{debug}");
    }

    #[test]
    fn only_allowlisted_hosts_become_stored_links() {
        let text = "uploaded new release 1 (1) successfully!\n\
                    View this release in the Firebase console: https://evil.example/console.firebase.google.com\n\
                    Share this release with testers who have access: http://appdistribution.firebase.google.com/x";
        match interpret_distribution(ok(0, text, ""), DISTRIBUTE_TIMEOUT, &[]) {
            RemoteOutcome::Confirmed { release } => {
                assert_eq!(release.console_uri, None);
                assert_eq!(release.testing_uri, None);
            }
            other => panic!("{other:?}"),
        }
    }
}
