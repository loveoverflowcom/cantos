//! Pure resolution of a distribution request into a typed plan. Nothing here reads the
//! environment, touches a file or starts a process: the shell passes facts in, and tests assert
//! the exact decisions. A value that exists has already been validated, so later stages cannot be
//! handed an unavailable channel/artifact combination or an unaddressed distribution.

use std::path::PathBuf;

use clap::ValueEnum;

use crate::build::Refusal;

/// The only application identity the demo channel may publish (`androidApp/build.gradle.kts`).
pub const DEMO_PACKAGE: &str = "com.cantos.theatre.demo";
/// The demo channel is a distribution policy; Phase A maps it onto the existing debug variant.
pub const DEMO_VARIANT: &str = "debug";
pub const DEMO_GRADLE_TASK: &str = ":androidApp:assembleDebug";
/// Version-name label that marks the build as a demo, never a production release.
pub const DEMO_VERSION_SUFFIX: &str = "-demo";
pub const APP_ID_ENV: &str = "CANTOS_FIREBASE_DEMO_APP_ID";
pub const CERT_ENV: &str = "CANTOS_FIREBASE_DEMO_CERT_SHA256";
/// Largest accepted release-notes text, in bytes (a local bound, not a service claim).
pub const NOTES_LIMIT: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Target {
    /// The Theatre Android demo host (Studio is Web-only and is not a target).
    CantosTheatre,
}

/// Distribution policy. Distinct from the Android build type/flavor it currently maps onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Channel {
    Demo,
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ArtifactKind {
    Apk,
    Aab,
}

impl Channel {
    fn label(self) -> &'static str {
        match self {
            Channel::Demo => "demo",
            Channel::Production => "production",
        }
    }
}

impl ArtifactKind {
    fn label(self) -> &'static str {
        match self {
            ArtifactKind::Apk => "apk",
            ArtifactKind::Aab => "aab",
        }
    }
}

/// The one supported channel/artifact policy. Other combinations are refused before this exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    DemoApk,
}

impl Policy {
    pub fn channel(self) -> &'static str {
        "demo"
    }

    pub fn artifact(self) -> &'static str {
        "apk"
    }

    pub fn variant(self) -> &'static str {
        DEMO_VARIANT
    }

    pub fn gradle_task(self) -> &'static str {
        DEMO_GRADLE_TASK
    }

    pub fn package(self) -> &'static str {
        DEMO_PACKAGE
    }
}

/// A Firebase Android app ID (`1:<project number>:android:<hex>`). Not a package name or a
/// Firebase project ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppId(String);

impl AppId {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut parts = text.split(':');
        let valid = matches!(
            (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()),
            (Some("1"), Some(number), Some("android"), Some(hex), None)
                if !number.is_empty()
                    && number.bytes().all(|b| b.is_ascii_digit())
                    && !hex.is_empty()
                    && hex.bytes().all(|b| b.is_ascii_hexdigit())
        );
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(format!(
                "`{text}` is not a Firebase Android app ID (expected 1:<project number>:android:<hex>); \
                 it is not a package name or a Firebase project ID"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppIdSource {
    Flag,
    Environment,
}

impl AppIdSource {
    pub fn label(self) -> &'static str {
        match self {
            AppIdSource::Flag => "--app-id",
            AppIdSource::Environment => APP_ID_ENV,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIdResolution {
    Resolved {
        id: AppId,
        source: AppIdSource,
    },
    /// Only a dry run may carry this; a live run refuses before any tool starts.
    Unresolved,
}

/// At least one explicit group alias or tester; never an implicit "all testers".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipients {
    groups: Vec<String>,
    testers: Vec<String>,
}

impl Recipients {
    pub fn new(groups: &[String], testers: &[String]) -> Result<Self, String> {
        let groups = normalize("--groups", groups, valid_group)?;
        let testers = normalize("--testers", testers, valid_email)?;
        if groups.is_empty() && testers.is_empty() {
            return Err(
                "name at least one recipient with --groups or --testers; there is no implicit all-testers default"
                    .to_owned(),
            );
        }
        Ok(Self { groups, testers })
    }

    pub fn groups(&self) -> &[String] {
        &self.groups
    }

    /// Never rendered or logged; the receipt records only their count.
    pub fn testers(&self) -> &[String] {
        &self.testers
    }
}

/// Trim, reject empty/invalid entries and deduplicate while keeping the caller's order.
fn normalize(
    flag: &str,
    entries: &[String],
    valid: fn(&str) -> bool,
) -> Result<Vec<String>, String> {
    let mut seen = Vec::<String>::new();
    for raw in entries {
        let entry = raw.trim();
        if entry.is_empty() {
            return Err(format!("{flag} contains an empty entry"));
        }
        if !valid(entry) {
            return Err(format!("{flag} contains an invalid entry"));
        }
        if !seen.iter().any(|known| known == entry) {
            seen.push(entry.to_owned());
        }
    }
    Ok(seen)
}

fn valid_group(alias: &str) -> bool {
    !alias.starts_with('-')
        && alias.len() <= 128
        && alias
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    email.len() <= 254
        && !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, ',' | ';' | '"' | '\'' | '<' | '>')
        })
}

/// A SHA-256 certificate fingerprint, normalized to 64 lowercase hex digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertFingerprint(String);

impl CertFingerprint {
    pub fn parse(text: &str) -> Result<Self, String> {
        let hex: String = text
            .trim()
            .chars()
            .filter(|c| *c != ':')
            .map(|c| c.to_ascii_lowercase())
            .collect();
        if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(Self(hex))
        } else {
            Err("expected certificate fingerprint must be 64 hex digits (SHA-256), optionally colon separated".to_owned())
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotesChoice {
    /// A demo-labeled summary of version/source identity, produced after the build.
    Generated,
    Inline(String),
    File {
        path: PathBuf,
        text: String,
    },
}

impl NotesChoice {
    pub fn describe(&self) -> String {
        match self {
            NotesChoice::Generated => "generated demo summary (version and source identity)".into(),
            NotesChoice::Inline(text) => format!("inline ({} bytes)", text.len()),
            NotesChoice::File { path, text } => {
                format!("file {} ({} bytes)", path.display(), text.len())
            }
        }
    }
}

/// What the shell observed before planning. Absent values stay absent; the plan decides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Configuration {
    pub app_id: Option<String>,
    pub cert_sha256: Option<String>,
}

/// The raw, already-parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
    pub target: Target,
    pub channel: Channel,
    pub artifact: ArtifactKind,
    pub app_id: Option<String>,
    pub groups: Vec<String>,
    pub testers: Vec<String>,
    pub release_notes: Option<String>,
    pub release_notes_file: Option<PathBuf>,
    pub dry_run: bool,
}

/// A fully validated request. Reaching one means the combination is supported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub policy: Policy,
    pub app_id: AppIdResolution,
    pub recipients: Recipients,
    pub notes: NotesChoice,
    pub expected_cert: Option<CertFingerprint>,
    pub dry_run: bool,
}

/// A request that may execute: its destination is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRequest {
    pub policy: Policy,
    pub app_id: AppId,
    pub app_id_source: AppIdSource,
    pub recipients: Recipients,
    pub notes: NotesChoice,
    pub expected_cert: Option<CertFingerprint>,
}

impl Request {
    pub fn live(&self) -> Result<LiveRequest, Refusal> {
        match &self.app_id {
            AppIdResolution::Resolved { id, source } => Ok(LiveRequest {
                policy: self.policy,
                app_id: id.clone(),
                app_id_source: *source,
                recipients: self.recipients.clone(),
                notes: self.notes.clone(),
                expected_cert: self.expected_cert.clone(),
            }),
            AppIdResolution::Unresolved => Err(Refusal::Usage(format!(
                "no Firebase app ID: pass --app-id or export {APP_ID_ENV}; there is no implicit default project or app"
            ))),
        }
    }
}

fn usage(message: impl Into<String>) -> Refusal {
    Refusal::Usage(message.into())
}

/// Validate every fact and produce the plan, or the first refusal. Unavailable combinations are
/// refused as `UNSUPPORTED` before any other fact matters.
pub fn resolve(
    raw: &Raw,
    configuration: &Configuration,
    read_notes_file: impl FnOnce(&std::path::Path) -> std::io::Result<Vec<u8>>,
) -> Result<Request, Refusal> {
    let Target::CantosTheatre = raw.target;
    let policy = match (raw.channel, raw.artifact) {
        (Channel::Demo, ArtifactKind::Apk) => Policy::DemoApk,
        (channel, artifact) => {
            return Err(Refusal::Unsupported {
                subject: format!(
                    "distribute firebase {}+{}",
                    channel.label(),
                    artifact.label()
                ),
                reason: unavailable_reason(channel, artifact),
            })
        }
    };

    let recipients = Recipients::new(&raw.groups, &raw.testers).map_err(usage)?;

    let app_id = match (&raw.app_id, &configuration.app_id) {
        (Some(flag), _) => AppIdResolution::Resolved {
            id: AppId::parse(flag.trim()).map_err(usage)?,
            source: AppIdSource::Flag,
        },
        (None, Some(variable)) => AppIdResolution::Resolved {
            id: AppId::parse(variable.trim())
                .map_err(|message| usage(format!("{APP_ID_ENV}: {message}")))?,
            source: AppIdSource::Environment,
        },
        (None, None) => AppIdResolution::Unresolved,
    };

    let expected_cert = configuration
        .cert_sha256
        .as_deref()
        .map(|value| CertFingerprint::parse(value).map_err(|m| usage(format!("{CERT_ENV}: {m}"))))
        .transpose()?;

    let notes = match (&raw.release_notes, &raw.release_notes_file) {
        (Some(_), Some(_)) => {
            return Err(usage(
                "--release-notes and --release-notes-file are mutually exclusive",
            ))
        }
        (Some(text), None) => NotesChoice::Inline(checked_notes("--release-notes", text.clone())?),
        (None, Some(path)) => {
            let bytes = read_notes_file(path).map_err(|error| {
                usage(format!(
                    "cannot read --release-notes-file: {}",
                    error.kind()
                ))
            })?;
            let text = String::from_utf8(bytes)
                .map_err(|_| usage("--release-notes-file must be valid UTF-8 text"))?;
            NotesChoice::File {
                path: path.clone(),
                text: checked_notes("--release-notes-file", text)?,
            }
        }
        (None, None) => NotesChoice::Generated,
    };

    Ok(Request {
        policy,
        app_id,
        recipients,
        notes,
        expected_cert,
        dry_run: raw.dry_run,
    })
}

fn unavailable_reason(channel: Channel, artifact: ArtifactKind) -> String {
    match (channel, artifact) {
        (Channel::Demo, ArtifactKind::Aab) => {
            "demo AAB distribution is not implemented: the demo host is a mock APK and App Distribution AAB \
             needs a Google Play link; use --artifact apk"
                .to_owned()
        }
        _ => "production distribution is unavailable: no production Android host exists, the production \
             release graph is not yet shown to exclude sandbox code (#122), and no release signing or \
             isolated Firebase app is configured; a demo build is never a fallback"
            .to_owned(),
    }
}

fn checked_notes(flag: &str, text: String) -> Result<String, Refusal> {
    if text.trim().is_empty() {
        return Err(usage(format!(
            "{flag} is empty; omit it to get the generated demo summary"
        )));
    }
    if text.contains('\0') {
        return Err(usage(format!("{flag} contains a NUL byte")));
    }
    if text.len() > NOTES_LIMIT {
        return Err(usage(format!(
            "{flag} is {} bytes; the limit is {NOTES_LIMIT}",
            text.len()
        )));
    }
    Ok(text)
}

/// The generated demo-labeled summary. Pure: version and source facts are arguments.
pub fn generated_notes(
    version_name: &str,
    version_code: &str,
    commit: &str,
    dirty: bool,
) -> String {
    let short: String = commit.chars().take(12).collect();
    let tree = if dirty { " (uncommitted changes)" } else { "" };
    format!(
        "Cantos Theatre DEMO build: a mock listener UI for QA, not a production release.\n\
         Version: {version_name} ({version_code})\n\
         Source: {short}{tree}\n"
    )
}

/// Stages in execution order. The report and receipt use these names so a reader sees what was
/// reached and what was not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Prerequisites,
    FirebaseCli,
    RunRecord,
    Destination,
    Build,
    Inspect,
    IntentReceipt,
    Distribution,
    FinalReceipt,
}

impl Stage {
    pub const ALL: [Stage; 9] = [
        Stage::Prerequisites,
        Stage::FirebaseCli,
        Stage::RunRecord,
        Stage::Destination,
        Stage::Build,
        Stage::Inspect,
        Stage::IntentReceipt,
        Stage::Distribution,
        Stage::FinalReceipt,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::Prerequisites => "local prerequisites",
            Stage::FirebaseCli => "Firebase CLI",
            Stage::RunRecord => "run record",
            Stage::Destination => "Firebase destination",
            Stage::Build => "Android build",
            Stage::Inspect => "APK inspection and staging",
            Stage::IntentReceipt => "intent receipt",
            Stage::Distribution => "Firebase distribution",
            Stage::FinalReceipt => "final receipt",
        }
    }

    fn plan_note(self) -> &'static str {
        match self {
            Stage::Prerequisites => {
                "installed Gradle, JDK, Android SDK platform/build-tools; nothing is installed"
            }
            Stage::FirebaseCli => {
                "`firebase --version` must report a documented compatible release"
            }
            Stage::RunRecord => "create the private run directory; record the plan",
            Stage::Destination => {
                "read-only `apps:sdkconfig`; the registered package must equal the demo package"
            }
            Stage::Build => "one Gradle build through the existing Android plan",
            Stage::Inspect => {
                "exactly one output; package, version, debuggable, signature, SHA-256; staged copy"
            }
            Stage::IntentReceipt => "record intent before the remote write",
            Stage::Distribution => {
                "REMOTE WRITE: upload, release notes, distribute (may notify testers)"
            }
            Stage::FinalReceipt => "record the observed outcome and release links",
        }
    }
}

/// Everything the dry run prints, with the already-rendered tool invocations supplied by the
/// caller. Pure text; no tool is probed.
pub fn render_plan(request: &Request, gradle: &str, firebase: &str) -> String {
    let mut text = String::from(
        "distribute firebase plan (dry run: nothing was probed, built, uploaded or written)\n",
    );
    let mut row = |name: &str, value: String| text.push_str(&format!("  {name:<18} {value}\n"));
    row("target", "cantos-theatre (Android demo host)".to_owned());
    row(
        "channel",
        format!(
            "{} (a distribution policy; not a production claim)",
            request.policy.channel()
        ),
    );
    row("artifact", request.policy.artifact().to_owned());
    row(
        "Gradle variant",
        format!(
            "{} via {} (no Gradle flavor named demo exists)",
            request.policy.variant(),
            request.policy.gradle_task()
        ),
    );
    row("expected package", request.policy.package().to_owned());
    row(
        "Firebase app ID",
        match &request.app_id {
            AppIdResolution::Resolved { id, source } => {
                format!("{} (from {})", id.as_str(), source.label())
            }
            AppIdResolution::Unresolved => {
                format!("UNRESOLVED (a live run needs --app-id or {APP_ID_ENV})")
            }
        },
    );
    row("groups", join_or(request.recipients.groups(), "none"));
    row(
        "testers",
        format!(
            "{} explicit (addresses are never printed)",
            request.recipients.testers().len()
        ),
    );
    row("release notes", request.notes.describe());
    row(
        "signing",
        match &request.expected_cert {
            Some(cert) => format!(
                "debug certificate must match {} (from {CERT_ENV})",
                cert.as_str()
            ),
            None => format!("fingerprint recorded; no expectation configured ({CERT_ENV} unset)"),
        },
    );
    text.push_str("  stages\n");
    for (index, stage) in Stage::ALL.iter().enumerate() {
        text.push_str(&format!(
            "    {}. {:<28} {}\n",
            index + 1,
            stage.label(),
            stage.plan_note()
        ));
    }
    text.push_str(&format!(
        "  gradle    $ {gradle}\n  firebase  $ {firebase}\n"
    ));
    text.push_str(
        "NOT_RUN distribute firebase: a plan is not an authenticated readiness check or a distribution.\n",
    );
    text
}

fn join_or(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        empty.to_owned()
    } else {
        values.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "1:1234567890:android:0a1b2c3d4e5f67890";

    fn raw() -> Raw {
        Raw {
            target: Target::CantosTheatre,
            channel: Channel::Demo,
            artifact: ArtifactKind::Apk,
            app_id: Some(APP.into()),
            groups: vec!["cantos-internal".into()],
            testers: vec![],
            release_notes: None,
            release_notes_file: None,
            dry_run: false,
        }
    }

    fn resolve_with(raw: &Raw, configuration: &Configuration) -> Result<Request, Refusal> {
        resolve(raw, configuration, |_| Ok(b"notes from file".to_vec()))
    }

    fn usage_text(result: Result<Request, Refusal>) -> String {
        match result {
            Err(Refusal::Usage(message)) => message,
            other => panic!("expected a usage refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_supported_request_resolves_to_the_demo_apk_policy() {
        let request = resolve_with(&raw(), &Configuration::default()).unwrap();
        assert_eq!(request.policy, Policy::DemoApk);
        assert_eq!(request.policy.variant(), "debug");
        assert_eq!(request.policy.gradle_task(), ":androidApp:assembleDebug");
        assert_eq!(request.notes, NotesChoice::Generated);
        assert!(!request.dry_run);
    }

    #[test]
    fn unavailable_combinations_are_unsupported_before_any_other_fact_matters() {
        for (channel, artifact) in [
            (Channel::Demo, ArtifactKind::Aab),
            (Channel::Production, ArtifactKind::Apk),
            (Channel::Production, ArtifactKind::Aab),
        ] {
            // Even a request with no recipients and an invalid app ID is UNSUPPORTED first.
            let bad = Raw {
                channel,
                artifact,
                app_id: Some("not-an-app-id".into()),
                groups: vec![],
                ..raw()
            };
            match resolve_with(&bad, &Configuration::default()) {
                Err(Refusal::Unsupported { subject, reason }) => {
                    assert!(subject.contains(channel.label()), "{subject}");
                    assert!(!reason.is_empty());
                }
                other => panic!("{channel:?}+{artifact:?}: {other:?}"),
            }
        }
        // No silent fallback to demo for production.
        let production = resolve_with(
            &Raw {
                channel: Channel::Production,
                ..raw()
            },
            &Configuration::default(),
        );
        assert!(matches!(production, Err(Refusal::Unsupported { .. })));
    }

    #[test]
    fn recipients_are_trimmed_deduplicated_and_never_empty() {
        let request = resolve_with(
            &Raw {
                groups: vec![" qa ".into(), "qa".into(), "ops-team".into()],
                testers: vec!["a@example.com".into(), "a@example.com".into()],
                ..raw()
            },
            &Configuration::default(),
        )
        .unwrap();
        assert_eq!(request.recipients.groups(), ["qa", "ops-team"]);
        assert_eq!(request.recipients.testers(), ["a@example.com"]);

        let testers_only = Raw {
            groups: vec![],
            testers: vec!["qa@example.com".into()],
            ..raw()
        };
        assert!(resolve_with(&testers_only, &Configuration::default()).is_ok());

        let none = Raw {
            groups: vec![],
            ..raw()
        };
        assert!(usage_text(resolve_with(&none, &Configuration::default()))
            .contains("at least one recipient"));
    }

    #[test]
    fn empty_or_malformed_recipient_entries_are_rejected() {
        for groups in [
            vec!["".to_owned()],
            vec!["a".into(), "  ".into()],
            vec!["-x".into()],
            vec!["a b".into()],
        ] {
            let message = usage_text(resolve_with(
                &Raw { groups, ..raw() },
                &Configuration::default(),
            ));
            assert!(message.contains("--groups"), "{message}");
        }
        for tester in [
            "",
            "plain",
            "@example.com",
            "a@",
            "a@b",
            "a@@b.c",
            "a b@c.d",
            "a@b.c,d@e.f",
            "a@.c.",
        ] {
            let message = usage_text(resolve_with(
                &Raw {
                    testers: vec![tester.into()],
                    ..raw()
                },
                &Configuration::default(),
            ));
            assert!(message.contains("--testers"), "{tester}: {message}");
            assert!(
                !message.contains(tester) || tester.is_empty(),
                "echoes an address: {message}"
            );
        }
    }

    #[test]
    fn the_flag_overrides_the_environment_and_neither_has_a_default() {
        let environment = Configuration {
            app_id: Some("1:999:android:abc".into()),
            ..Default::default()
        };
        let flagged = resolve_with(&raw(), &environment).unwrap();
        assert!(matches!(
            flagged.app_id,
            AppIdResolution::Resolved { ref id, source: AppIdSource::Flag } if id.as_str() == APP
        ));
        let from_environment = resolve_with(
            &Raw {
                app_id: None,
                ..raw()
            },
            &environment,
        )
        .unwrap();
        assert!(matches!(
            from_environment.app_id,
            AppIdResolution::Resolved { ref id, source: AppIdSource::Environment } if id.as_str() == "1:999:android:abc"
        ));
        let neither = resolve_with(
            &Raw {
                app_id: None,
                ..raw()
            },
            &Configuration::default(),
        )
        .unwrap();
        assert_eq!(neither.app_id, AppIdResolution::Unresolved);
        assert!(usage_text(neither.live().map(|_| neither.clone())).contains(APP_ID_ENV));
    }

    #[test]
    fn app_ids_are_checked_in_both_sources_and_names_are_not_app_ids() {
        for bad in [
            "com.cantos.theatre.demo",
            "cantos-a348b",
            "1:123:ios:abc",
            "1:12:android:xyz",
            "1::android:ab",
            "2:1:android:ab",
            "1:1:android:ab:cd",
        ] {
            assert!(AppId::parse(bad).is_err(), "{bad}");
            let message = usage_text(resolve_with(
                &Raw {
                    app_id: Some(bad.into()),
                    ..raw()
                },
                &Configuration::default(),
            ));
            assert!(message.contains("app ID"), "{message}");
        }
        let message = usage_text(resolve_with(
            &Raw {
                app_id: None,
                ..raw()
            },
            &Configuration {
                app_id: Some("garbage".into()),
                ..Default::default()
            },
        ));
        assert!(message.contains(APP_ID_ENV), "{message}");
    }

    #[test]
    fn notes_choices_conflict_stay_bounded_and_preserve_text() {
        let both = Raw {
            release_notes: Some("a".into()),
            release_notes_file: Some("n.txt".into()),
            ..raw()
        };
        assert!(usage_text(resolve_with(&both, &Configuration::default()))
            .contains("mutually exclusive"));

        let multiline = "Dòng một — thử nghiệm\nDòng hai: Ngày mai, mình có diễn tiếp không?\n";
        let inline = resolve_with(
            &Raw {
                release_notes: Some(multiline.into()),
                ..raw()
            },
            &Configuration::default(),
        )
        .unwrap();
        assert_eq!(inline.notes, NotesChoice::Inline(multiline.into()));

        let spaced = std::path::PathBuf::from("/tmp/path with spaces/ghi chú.txt");
        let file = resolve(
            &Raw {
                release_notes_file: Some(spaced.clone()),
                ..raw()
            },
            &Configuration::default(),
            |path| {
                assert_eq!(path, spaced);
                Ok(multiline.as_bytes().to_vec())
            },
        )
        .unwrap();
        assert_eq!(
            file.notes,
            NotesChoice::File {
                path: spaced,
                text: multiline.into()
            }
        );

        for bad in ["", "   \n", "nul\0byte"] {
            assert!(matches!(
                resolve_with(
                    &Raw {
                        release_notes: Some(bad.into()),
                        ..raw()
                    },
                    &Configuration::default()
                ),
                Err(Refusal::Usage(_))
            ));
        }
        let long = "x".repeat(NOTES_LIMIT + 1);
        assert!(matches!(
            resolve_with(
                &Raw {
                    release_notes: Some(long),
                    ..raw()
                },
                &Configuration::default()
            ),
            Err(Refusal::Usage(_))
        ));
        let invalid_utf8 = resolve(
            &Raw {
                release_notes_file: Some("n.txt".into()),
                ..raw()
            },
            &Configuration::default(),
            |_| Ok(vec![0xff, 0xfe]),
        );
        assert!(usage_text(invalid_utf8).contains("UTF-8"));
        let unreadable = resolve(
            &Raw {
                release_notes_file: Some("n.txt".into()),
                ..raw()
            },
            &Configuration::default(),
            |_| Err(std::io::ErrorKind::NotFound.into()),
        );
        assert!(usage_text(unreadable).contains("cannot read"));
    }

    #[test]
    fn a_configured_certificate_expectation_is_normalized_or_refused() {
        let colon = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";
        let request = resolve_with(
            &raw(),
            &Configuration {
                cert_sha256: Some(colon.into()),
                ..Default::default()
            },
        )
        .unwrap();
        let cert = request.expected_cert.unwrap();
        assert_eq!(cert.as_str().len(), 64);
        assert!(!cert.as_str().contains(':'));
        assert_eq!(cert.as_str(), cert.as_str().to_ascii_lowercase());
        let message = usage_text(resolve_with(
            &raw(),
            &Configuration {
                cert_sha256: Some("abc".into()),
                ..Default::default()
            },
        ));
        assert!(message.contains(CERT_ENV));
    }

    #[test]
    fn the_generated_summary_is_demo_labeled_and_carries_identity() {
        let text = generated_notes("0.1.0-demo", "1", "0123456789abcdef0123", true);
        assert!(text.contains("DEMO"));
        assert!(text.contains("not a production release"));
        assert!(text.contains("0.1.0-demo (1)"));
        assert!(text.contains("0123456789ab (uncommitted changes)"));
        assert!(!generated_notes("v", "1", "abc", false).contains("uncommitted"));
    }

    #[test]
    fn the_dry_run_plan_is_explicit_about_unresolved_configuration_and_hides_addresses() {
        let request = resolve_with(
            &Raw {
                app_id: None,
                testers: vec!["secret.person@example.com".into()],
                dry_run: true,
                ..raw()
            },
            &Configuration::default(),
        )
        .unwrap();
        let plan = render_plan(&request, "gradle ...", "firebase ...");
        assert!(plan.contains("UNRESOLVED"));
        assert!(plan.contains("1 explicit"));
        assert!(!plan.contains("secret.person"));
        assert!(plan.contains("REMOTE WRITE"));
        assert!(plan.contains("no Gradle flavor named demo"));
        assert!(plan.contains("NOT_RUN"));
        assert!(plan.contains("unset"));
        for stage in Stage::ALL {
            assert!(plan.contains(stage.label()), "{}", stage.label());
        }
    }
}
