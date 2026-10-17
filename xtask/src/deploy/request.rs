//! Pure resolution of a Hosting deploy request into a typed plan. Nothing here reads the
//! environment, touches a file or starts a process: the shell passes facts in, and tests assert
//! the exact decisions. A value that exists has already been validated, so later stages cannot be
//! handed a malformed project, an ambiguous channel, a live-like spelling the Firebase CLI would
//! normalize elsewhere, or an expiry on a live release.
//!
//! Four identifiers are different things and are never interchangeable: the **Firebase project
//! ID** (`--project`), the **Hosting site ID** (resolved from the operator's `.firebaserc`), the
//! **deploy target** (`theatre` / `studio`, a local alias) and an Android Firebase App ID.

use clap::ValueEnum;

use super::hosting_config::{Resolution, SiteId};
use crate::{
    build::{self, Refusal, Surface},
    config::App,
};

/// The Hosting deploy target of an app. It is the key in `firebase.json` and `.firebaserc`, not a
/// site ID.
pub fn hosting_target(app: App) -> &'static str {
    match app {
        App::Theatre => "theatre",
        App::Studio => "studio",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HostedApp {
    /// Cantos Theatre Web (listeners): Hosting target `theatre`.
    CantosTheatre,
    /// Cantos Studio Web (creators): Hosting target `studio`.
    CantosStudio,
}

impl HostedApp {
    pub fn app(self) -> App {
        match self {
            HostedApp::CantosTheatre => App::Theatre,
            HostedApp::CantosStudio => App::Studio,
        }
    }
}

/// A Firebase project ID: 6–30 characters, lowercase letters, digits and hyphens, starting with
/// a letter and not ending with a hyphen. Not a project name, a project number or a site ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectId(String);

impl ProjectId {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.contains(':') {
            return Err(format!(
                "`{text}` is not a Firebase project ID (it looks like an App ID, which names an \
                 Android/web app, not a project)"
            ));
        }
        if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!(
                "`{text}` is a project number; --project takes the project ID (for example \
                 `my-cantos-prod`), shown by `firebase projects:list`"
            ));
        }
        let valid = (6..=30).contains(&text.len())
            && text.starts_with(|c: char| c.is_ascii_lowercase())
            && !text.ends_with('-')
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(format!(
                "`{text}` is not a Firebase project ID (6-30 characters: lowercase letters, \
                 digits and hyphens, starting with a letter, not ending with a hyphen); a project \
                 display name is not accepted"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A preview channel ID. Only spellings the Firebase CLI keeps unchanged are accepted: the CLI
/// rewrites `/`, `:`, `_` and `#` to `-` and treats any case or whitespace variant of `live` as
/// the live channel, so those spellings would silently name another destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewChannel(String);

impl PreviewChannel {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Channel {
    /// The site's live release. Always an explicit, deliberate choice.
    Live,
    Preview(PreviewChannel),
}

/// Hosting channel IDs are DNS-label sized; a stricter service-side limit is reported as a
/// refusal by the service, not guessed here.
// ACCEPTANCE(NOT_RUN): the Hosting service's own channel-ID limits were not exercised against a
// real project. Owner: the operator running the first preview deploy (docs/deployment).
const CHANNEL_LIMIT: usize = 63;

impl Channel {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text == "live" {
            return Ok(Channel::Live);
        }
        if text.trim().eq_ignore_ascii_case("live") {
            return Err(format!(
                "`{text}` would be treated as the live channel by the Firebase CLI; a live deploy \
                 must be spelled exactly `live`"
            ));
        }
        let valid = !text.is_empty()
            && text.len() <= CHANNEL_LIMIT
            && !text.starts_with('-')
            && !text.ends_with('-')
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if valid {
            return Ok(Channel::Preview(PreviewChannel(text.to_owned())));
        }
        let rewritten = text.contains(['/', ':', '_', '#']);
        Err(if rewritten {
            format!(
                "`{text}` contains a character (`/ : _ #`) that the Firebase CLI silently rewrites \
                 to `-`, which would deploy to a different channel; use only lowercase letters, \
                 digits and hyphens"
            )
        } else {
            format!(
                "`{text}` is not a preview channel ID (1-{CHANNEL_LIMIT} characters: lowercase \
                 letters, digits and hyphens, not starting or ending with a hyphen)"
            )
        })
    }

    pub fn label(&self) -> &str {
        match self {
            Channel::Live => "live",
            Channel::Preview(channel) => channel.as_str(),
        }
    }

    pub fn is_live(&self) -> bool {
        matches!(self, Channel::Live)
    }
}

/// A preview-channel lifetime in the Firebase CLI's own syntax (`30m`, `12h`, `7d`), at most 30
/// days and never zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expiry {
    text: String,
    minutes: u64,
}

pub const EXPIRY_LIMIT_DAYS: u64 = 30;
pub const EXPIRY_DEFAULT_DAYS: u64 = 7;

impl Expiry {
    pub fn parse(text: &str) -> Result<Self, String> {
        let bad = || {
            format!(
                "`{text}` is not an expiry: use digits and one unit, such as 30m, 12h or 7d \
                 (at most {EXPIRY_LIMIT_DAYS}d)"
            )
        };
        let unit = text.chars().last().ok_or_else(bad)?;
        let digits = &text[..text.len() - unit.len_utf8()];
        let per_unit = match unit {
            'm' => 1,
            'h' => 60,
            'd' => 24 * 60,
            _ => return Err(bad()),
        };
        if digits.is_empty() || digits.len() > 9 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(bad());
        }
        let amount: u64 = digits.parse().map_err(|_| bad())?;
        if amount == 0 {
            return Err(format!("`{text}` is zero; an expiry must be positive"));
        }
        let minutes = amount * per_unit;
        if minutes > EXPIRY_LIMIT_DAYS * 24 * 60 {
            return Err(format!(
                "`{text}` is longer than the {EXPIRY_LIMIT_DAYS}d limit of Firebase preview channels"
            ));
        }
        Ok(Self {
            text: text.to_owned(),
            minutes,
        })
    }

    pub fn as_arg(&self) -> &str {
        &self.text
    }

    #[cfg(test)]
    pub fn minutes(&self) -> u64 {
        self.minutes
    }
}

/// The raw, already-parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
    pub app: HostedApp,
    pub surface: Option<Surface>,
    pub project: String,
    pub channel: String,
    pub expires: Option<String>,
    pub dry_run: bool,
}

/// A fully validated request. Reaching one means the app, surface, project, channel and expiry are
/// each well formed and mutually consistent. The destination *site* is not part of it: that comes
/// from the operator's local mapping and is resolved separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub app: App,
    pub project: ProjectId,
    pub channel: Channel,
    pub expires: Option<Expiry>,
    pub dry_run: bool,
}

impl Request {
    pub fn target(&self) -> &'static str {
        hosting_target(self.app)
    }
}

fn usage(message: impl Into<String>) -> Refusal {
    Refusal::Usage(message.into())
}

/// Validate every fact and produce the request, or the first refusal. Surfaces other than the
/// app's Web client reuse the build command's refusals (Studio is web-only; Theatre mobile is
/// UNSUPPORTED).
pub fn resolve(raw: &Raw) -> Result<Request, Refusal> {
    let app = raw.app.app();
    build::app_surface(app, raw.surface)?;
    let project = ProjectId::parse(&raw.project).map_err(usage)?;
    let channel = Channel::parse(&raw.channel).map_err(usage)?;
    let expires = raw
        .expires
        .as_deref()
        .map(Expiry::parse)
        .transpose()
        .map_err(usage)?;
    if channel.is_live() && expires.is_some() {
        return Err(usage(
            "--expires applies only to a preview channel; a live release does not expire",
        ));
    }
    Ok(Request {
        app,
        project,
        channel,
        expires,
        dry_run: raw.dry_run,
    })
}

/// Stages in execution order. The report and receipt use these names so a reader sees what was
/// reached and what was not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Prerequisites,
    HostingConfig,
    FirebaseCli,
    RunRecord,
    Destination,
    Build,
    Bundle,
    IntentReceipt,
    Release,
    Smoke,
    FinalReceipt,
}

impl Stage {
    pub const ALL: [Stage; 11] = [
        Stage::Prerequisites,
        Stage::HostingConfig,
        Stage::FirebaseCli,
        Stage::RunRecord,
        Stage::Destination,
        Stage::Build,
        Stage::Bundle,
        Stage::IntentReceipt,
        Stage::Release,
        Stage::Smoke,
        Stage::FinalReceipt,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::Prerequisites => "local prerequisites",
            Stage::HostingConfig => "Hosting configuration",
            Stage::FirebaseCli => "Firebase CLI",
            Stage::RunRecord => "run record",
            Stage::Destination => "Hosting destination",
            Stage::Build => "release build",
            Stage::Bundle => "bundle validation",
            Stage::IntentReceipt => "intent receipt",
            Stage::Release => "Hosting release",
            Stage::Smoke => "HTTP smoke",
            Stage::FinalReceipt => "final receipt",
        }
    }

    fn plan_note(self) -> &'static str {
        match self {
            Stage::Prerequisites => {
                "installed Rust, wasm32 target, Trunk and Git; no inherited TRUNK_* override; nothing is installed"
            }
            Stage::HostingConfig => {
                "require an exact HTTPS CANTOS_API_ORIGIN; validate the tracked Hosting policy and the operator's one-site target mapping"
            }
            Stage::FirebaseCli => {
                "`firebase --version` must report a documented compatible release"
            }
            Stage::RunRecord => {
                "create the private run directory; write an effective config for this ONE target"
            }
            Stage::Destination => {
                "read-only `hosting:sites:list` in the explicit project; the mapped site must exist"
            }
            Stage::Build => "one locked release Trunk build of this app into the run's own dist",
            Stage::Bundle => {
                "validate the exact public bytes, compute a manifest, require them unchanged before upload"
            }
            Stage::IntentReceipt => "record intent before the remote write",
            Stage::Release => {
                "REMOTE WRITE: one preview-channel deploy or one live release; never retried"
            }
            Stage::Smoke => {
                "read-only HTTPS requests to the observed URL (static delivery only, not login or API)"
            }
            Stage::FinalReceipt => "record the observed outcome",
        }
    }
}

/// Everything the dry run prints, with the already-rendered tool invocations supplied by the
/// caller. Pure text; no tool is probed.
pub fn render_plan(
    request: &Request,
    resolution: &Resolution,
    build_command: &str,
    firebase_commands: &[(&str, String)],
) -> String {
    let mut text = String::from(
        "deploy firebase plan (dry run: nothing was probed, built, uploaded or written)\n",
    );
    let mut row = |name: &str, value: String| text.push_str(&format!("  {name:<18} {value}\n"));
    row("target", format!("{} (Web only)", request.app.cli_name()));
    row("project", request.project.as_str().to_owned());
    row("deploy target", request.target().to_owned());
    row(
        "site",
        match resolution {
            Resolution::Resolved { site } => {
                format!("{} (from .firebaserc)", site_text(site))
            }
            Resolution::Unresolved { reason } => format!("UNRESOLVED ({reason})"),
        },
    );
    row(
        "channel",
        match &request.channel {
            Channel::Live => "live (a deliberate LIVE release)".to_owned(),
            Channel::Preview(channel) => format!(
                "{} (a PUBLIC preview channel; not tester-gated)",
                channel.as_str()
            ),
        },
    );
    if !request.channel.is_live() {
        row(
            "expires",
            match &request.expires {
                Some(expiry) => expiry.as_arg().to_owned(),
                None => format!(
                    "CLI default ({EXPIRY_DEFAULT_DAYS}d; an existing channel is extended to at least that)"
                ),
            },
        );
    }
    row(
        "serves",
        "static Leptos HTML/CSS/JS/WASM only; the API, PostgreSQL, Kanidm and storage stay outside Hosting".to_owned(),
    );
    text.push_str("  stages\n");
    for (index, stage) in Stage::ALL.iter().enumerate() {
        text.push_str(&format!(
            "    {:>2}. {:<28} {}\n",
            index + 1,
            stage.label(),
            stage.plan_note()
        ));
    }
    text.push_str(&format!("  build     $ {build_command}\n"));
    for (name, command) in firebase_commands {
        text.push_str(&format!("  {name:<9} $ {command}\n"));
    }
    text.push_str(
        "NOT_RUN deploy firebase: a plan is not an authenticated readiness check or a deployment.\n",
    );
    text
}

fn site_text(site: &SiteId) -> &str {
    site.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> Raw {
        Raw {
            app: HostedApp::CantosTheatre,
            surface: Some(Surface::Web),
            project: "cantos-example".into(),
            channel: "review-hosting".into(),
            expires: None,
            dry_run: false,
        }
    }

    #[test]
    fn each_app_maps_to_its_own_hosting_target() {
        assert_eq!(hosting_target(App::Theatre), "theatre");
        assert_eq!(hosting_target(App::Studio), "studio");
        let theatre = resolve(&raw()).unwrap();
        assert_eq!((theatre.app, theatre.target()), (App::Theatre, "theatre"));
        let studio = resolve(&Raw {
            app: HostedApp::CantosStudio,
            ..raw()
        })
        .unwrap();
        assert_eq!((studio.app, studio.target()), (App::Studio, "studio"));
    }

    #[test]
    fn a_surface_other_than_the_web_client_is_refused_like_the_build_command() {
        assert!(matches!(
            resolve(&Raw {
                surface: None,
                ..raw()
            }),
            Err(Refusal::Usage(message)) if message.contains("--web")
        ));
        assert!(matches!(
            resolve(&Raw {
                surface: Some(Surface::Mobile),
                ..raw()
            }),
            Err(Refusal::Unsupported { subject, .. }) if subject == "cantos-theatre --mobile"
        ));
        for surface in [None, Some(Surface::Mobile)] {
            assert!(matches!(
                resolve(&Raw {
                    app: HostedApp::CantosStudio,
                    surface,
                    ..raw()
                }),
                Err(Refusal::Usage(message)) if message.contains("web-only")
            ));
        }
    }

    #[test]
    fn project_ids_are_well_formed_and_never_names_numbers_or_app_ids() {
        for good in ["cantos-example", "abcdef", "a-b-c-d-e-f", "my-app-2026"] {
            assert_eq!(ProjectId::parse(good).unwrap().as_str(), good);
        }
        for bad in [
            "",
            "short",
            "Cantos-Example",
            "cantos example",
            "-cantos-example",
            "cantos-example-",
            "9cantos-example",
            "cantos_example",
            &"a".repeat(31),
        ] {
            assert!(ProjectId::parse(bad).is_err(), "{bad:?}");
        }
        assert!(ProjectId::parse("123456789012")
            .unwrap_err()
            .contains("project number"));
        assert!(ProjectId::parse("1:123456789012:android:0abc")
            .unwrap_err()
            .contains("App ID"));
    }

    #[test]
    fn live_is_exactly_the_word_live_and_never_a_spelling_variant() {
        assert_eq!(Channel::parse("live"), Ok(Channel::Live));
        for variant in ["Live", "LIVE", " live", "live ", "lIvE\t"] {
            let error = Channel::parse(variant).unwrap_err();
            assert!(error.contains("live channel"), "{variant:?}: {error}");
        }
        // A name that merely contains the word is an ordinary preview channel.
        assert!(matches!(
            Channel::parse("live-review"),
            Ok(Channel::Preview(_))
        ));
        assert!(matches!(
            Channel::parse("not-live"),
            Ok(Channel::Preview(_))
        ));
    }

    #[test]
    fn preview_channels_reject_spellings_the_cli_would_normalize_elsewhere() {
        for name in ["review-hosting", "pr-199", "a", "0", "x-1-y"] {
            assert_eq!(Channel::parse(name).unwrap().label(), name);
        }
        for rewritten in ["feature/x", "a:b", "snake_case", "issue#199"] {
            let error = Channel::parse(rewritten).unwrap_err();
            assert!(error.contains("silently rewrites"), "{rewritten}: {error}");
        }
        for bad in [
            "",
            "-lead",
            "trail-",
            "Upper",
            "with space",
            "dot.ted",
            "--flag",
            "ünï",
            &"a".repeat(64),
        ] {
            assert!(Channel::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Channel::parse(&"a".repeat(63)).is_ok());
    }

    #[test]
    fn expiry_is_the_clis_syntax_positive_and_at_most_thirty_days() {
        for (text, minutes) in [
            ("1m", 1),
            ("30m", 30),
            ("12h", 720),
            ("7d", 10_080),
            ("30d", 43_200),
            ("720h", 43_200),
            ("43200m", 43_200),
        ] {
            let expiry = Expiry::parse(text).unwrap();
            assert_eq!((expiry.as_arg(), expiry.minutes()), (text, minutes));
        }
        for bad in [
            "",
            "d",
            "7",
            "7D",
            "7 d",
            " 7d",
            "7d ",
            "-7d",
            "+7d",
            "1.5d",
            "P7D",
            "7w",
            "0d",
            "0m",
            "31d",
            "721h",
            "43201m",
            "9999999999d",
            "٣d",
        ] {
            assert!(Expiry::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Expiry::parse("0d").unwrap_err().contains("zero"));
    }

    #[test]
    fn an_expiry_on_a_live_release_is_a_usage_error() {
        let live = Raw {
            channel: "live".into(),
            expires: Some("7d".into()),
            ..raw()
        };
        assert!(matches!(
            resolve(&live),
            Err(Refusal::Usage(message)) if message.contains("preview channel")
        ));
        let preview = Raw {
            expires: Some("7d".into()),
            ..raw()
        };
        assert_eq!(resolve(&preview).unwrap().expires.unwrap().as_arg(), "7d");
        let live_only = Raw {
            channel: "live".into(),
            ..raw()
        };
        assert!(resolve(&live_only).unwrap().channel.is_live());
    }

    #[test]
    fn malformed_requests_are_usage_errors_not_unsupported() {
        for bad in [
            Raw {
                project: String::new(),
                ..raw()
            },
            Raw {
                project: "Not A Project".into(),
                ..raw()
            },
            Raw {
                channel: String::new(),
                ..raw()
            },
            Raw {
                channel: "feature/x".into(),
                ..raw()
            },
            Raw {
                expires: Some("forever".into()),
                ..raw()
            },
        ] {
            assert!(matches!(resolve(&bad), Err(Refusal::Usage(_))), "{bad:?}");
        }
    }

    #[test]
    fn the_plan_names_every_stage_the_destination_and_the_nothing_ran_line() {
        let request = resolve(&raw()).unwrap();
        let plan = render_plan(
            &request,
            &Resolution::Unresolved {
                reason: "no .firebaserc".into(),
            },
            "trunk build --locked --release",
            &[("firebase", "firebase deploy --only hosting:theatre".into())],
        );
        for stage in Stage::ALL {
            assert!(plan.contains(stage.label()), "{plan}");
        }
        assert!(plan.contains("UNRESOLVED (no .firebaserc)"));
        assert!(plan.contains("PUBLIC preview channel"));
        assert!(plan.contains("CLI default (7d"));
        assert!(plan.contains("NOT_RUN deploy firebase"));
        assert!(plan.contains("require an exact HTTPS CANTOS_API_ORIGIN"));
        assert!(!plan.contains("LIVE release"));
        let live = resolve(&Raw {
            channel: "live".into(),
            ..raw()
        })
        .unwrap();
        let plan = render_plan(
            &live,
            &Resolution::Unresolved { reason: "x".into() },
            "b",
            &[],
        );
        assert!(plan.contains("LIVE release"));
        assert!(!plan.contains("expires"));
    }
}
