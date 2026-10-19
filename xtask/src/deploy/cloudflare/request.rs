//! Pure resolution of a Cloudflare Pages deploy request. Nothing here reads the environment,
//! touches a file or starts a process.
//!
//! Five identifiers are different things and are never interchangeable: the **account ID**
//! (`--account`, 32 hex characters), the **Pages project name** (`--project`), the **branch**
//! (`--branch`; Pages derives production from it), the **deploy target** (`theatre` / `studio`,
//! a local alias) and the **release artifact** (`--artifact`, the verified output of
//! `release-build`). Production is never inferred from the working directory, the current Git
//! branch or a default account: the operator names the intent and the operator's own mapping
//! must agree with it before any file is extracted or uploaded.

use std::path::PathBuf;

use serde::Deserialize;

use crate::{
    build::{self, Refusal, Surface},
    config::App,
};

use super::super::request::{hosting_target, HostedApp};

/// A Cloudflare account ID: exactly 32 lowercase hexadecimal characters.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct AccountId(String);

impl AccountId {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() == 32 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Ok(Self(text.to_owned()))
        } else {
            Err(format!(
                "`{text}` is not a Cloudflare account ID (32 lowercase hexadecimal characters; \
                 it is neither an account name nor an e-mail address)"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AccountId {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        Self::parse(&text)
    }
}

/// A Pages project name: 1–58 characters, lowercase letters, digits and hyphens, not starting or
/// ending with a hyphen. It is not a Worker name, a zone or a hostname.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct ProjectName(String);

impl ProjectName {
    pub fn parse(text: &str) -> Result<Self, String> {
        let valid = (1..=58).contains(&text.len())
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !text.starts_with('-')
            && !text.ends_with('-');
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(format!(
                "`{text}` is not a Pages project name (1-58 characters: lowercase letters, digits \
                 and hyphens, no leading or trailing hyphen)"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ProjectName {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        Self::parse(&text)
    }
}

/// A Git branch name as Pages receives it. Deliberately narrower than Git: no whitespace,
/// control, glob or revision syntax, because the value reaches a command line and a record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Branch(String);

impl Branch {
    pub fn parse(text: &str) -> Result<Self, String> {
        let allowed = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/');
        let valid = (1..=100).contains(&text.len())
            && text.bytes().all(allowed)
            && !text.starts_with(['-', '/', '.'])
            && !text.ends_with(['/', '.'])
            && !text.contains("..")
            && !text.contains("//")
            && !text.ends_with(".lock");
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(format!(
                "`{text}` is not an accepted branch name (1-100 characters from A-Z a-z 0-9 . _ - /, \
                 not starting with `-`, `/` or `.`, not ending with `/`, `.` or `.lock`, and \
                 with no `..` or `//`)"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Branch {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        Self::parse(&text)
    }
}

/// What the operator intends the deployment to be. Pages makes a deployment *production* exactly
/// when its branch equals the project's production branch, so the intent must agree with the
/// branch; a disagreement is refused before any upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Production,
    Preview,
}

impl Environment {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "production" => Ok(Self::Production),
            "preview" => Ok(Self::Preview),
            other => Err(format!(
                "`{other}` is not an environment; name `production` or `preview` explicitly"
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Preview => "preview",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Raw {
    pub app: HostedApp,
    pub surface: Option<Surface>,
    pub account: String,
    pub project: String,
    pub branch: String,
    pub environment: String,
    pub artifact: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub app: App,
    pub account: AccountId,
    pub project: ProjectName,
    pub branch: Branch,
    pub environment: Environment,
    pub artifact: PathBuf,
    pub dry_run: bool,
}

impl Request {
    pub fn target(&self) -> &'static str {
        hosting_target(self.app)
    }

    /// The `release-build` component name: `theatre-web` or `studio-web`.
    pub fn component(&self) -> String {
        format!("{}-web", self.target())
    }
}

fn usage(message: impl Into<String>) -> Refusal {
    Refusal::Usage(message.into())
}

pub fn resolve(raw: &Raw) -> Result<Request, Refusal> {
    let app = raw.app.app();
    build::app_surface(app, raw.surface)?;
    let account = AccountId::parse(&raw.account).map_err(usage)?;
    let project = ProjectName::parse(&raw.project).map_err(usage)?;
    let branch = Branch::parse(&raw.branch).map_err(usage)?;
    let environment = Environment::parse(&raw.environment).map_err(usage)?;
    if raw.artifact.trim().is_empty() || raw.artifact.contains('\0') {
        return Err(usage(
            "--artifact must name the release-build output directory of this app",
        ));
    }
    Ok(Request {
        app,
        account,
        project,
        branch,
        environment,
        artifact: PathBuf::from(&raw.artifact),
        dry_run: raw.dry_run,
    })
}

/// The operator's reviewed destination for one app, from the private mapping.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Destination {
    pub project: ProjectName,
    pub production_branch: Branch,
}

/// Why a request does not agree with the operator's mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    Project {
        requested: String,
        mapped: String,
    },
    /// `production` was requested on a branch that is not the production branch.
    NotTheProductionBranch {
        branch: String,
        production: String,
    },
    /// `preview` was requested on the production branch, which Pages would publish live.
    WouldBeProduction {
        branch: String,
    },
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project { requested, mapped } => write!(
                f,
                "--project `{requested}` is not the project mapped to this app (`{mapped}`)"
            ),
            Self::NotTheProductionBranch { branch, production } => write!(
                f,
                "production was requested on branch `{branch}`, but the production branch is \
                 `{production}`; Pages would publish a preview instead"
            ),
            Self::WouldBeProduction { branch } => write!(
                f,
                "preview was requested on `{branch}`, which is the production branch: Pages would \
                 replace production"
            ),
        }
    }
}

/// Pure agreement check between the request and the mapped destination.
pub fn agree(request: &Request, destination: &Destination) -> Result<(), Mismatch> {
    if destination.project != request.project {
        return Err(Mismatch::Project {
            requested: request.project.as_str().to_owned(),
            mapped: destination.project.as_str().to_owned(),
        });
    }
    let on_production_branch = request.branch == destination.production_branch;
    match (request.environment, on_production_branch) {
        (Environment::Production, false) => Err(Mismatch::NotTheProductionBranch {
            branch: request.branch.as_str().to_owned(),
            production: destination.production_branch.as_str().to_owned(),
        }),
        (Environment::Preview, true) => Err(Mismatch::WouldBeProduction {
            branch: request.branch.as_str().to_owned(),
        }),
        _ => Ok(()),
    }
}

/// Stages in execution order, used by the report, the receipt and the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Prerequisites,
    Policy,
    Wrangler,
    Artifact,
    Staging,
    Bundle,
    Project,
    IntentReceipt,
    Publish,
    Readback,
    Smoke,
    FinalReceipt,
}

impl Stage {
    pub const ALL: [Stage; 12] = [
        Stage::Prerequisites,
        Stage::Policy,
        Stage::Wrangler,
        Stage::Artifact,
        Stage::Staging,
        Stage::Bundle,
        Stage::Project,
        Stage::IntentReceipt,
        Stage::Publish,
        Stage::Readback,
        Stage::Smoke,
        Stage::FinalReceipt,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::Prerequisites => "Prerequisites",
            Stage::Policy => "Policy and mapping",
            Stage::Wrangler => "Wrangler",
            Stage::Artifact => "Release artifact",
            Stage::Staging => "Staging",
            Stage::Bundle => "Bundle",
            Stage::Project => "Project lookup",
            Stage::IntentReceipt => "Intent receipt",
            Stage::Publish => "Publish",
            Stage::Readback => "Readback",
            Stage::Smoke => "HTTP smoke",
            Stage::FinalReceipt => "Final receipt",
        }
    }

    pub fn plan_note(self) -> &'static str {
        match self {
            Stage::Prerequisites => "node, curl and a CLOUDFLARE_API_TOKEN (name only) are present; no login",
            Stage::Policy => "tracked headers/404 policy and the private account/project/branch mapping agree with the request",
            Stage::Wrangler => "the pinned local Wrangler reports the exact pinned version; no npx download",
            Stage::Artifact => "manifest.json is strict and the archive bytes match its recorded SHA-256 and size",
            Stage::Staging => "extract the archive into a new run-owned directory with entry-by-entry checks",
            Stage::Bundle => "add the tracked policy files, validate the exact public bytes, limits and routes",
            Stage::Project => "READ: the project exists, is Direct Upload and is the mapped one",
            Stage::IntentReceipt => "record intent before the remote write",
            Stage::Publish => "REMOTE WRITE: one `pages deploy` of the staged directory; never retried",
            Stage::Readback => "READ: the reported deployment ID is listed in the right environment",
            Stage::Smoke => "read-only HTTPS requests to the deployment URL (static delivery only)",
            Stage::FinalReceipt => "record the observed outcome",
        }
    }
}

/// The reviewable plan. Pure text; no tool is probed.
pub fn render_plan(
    request: &Request,
    mapping: &Result<Destination, String>,
    commands: &[(&str, String)],
) -> String {
    let mut text = String::from(
        "deploy cloudflare plan (dry run: nothing was probed, extracted, uploaded or written)\n",
    );
    let mut row = |name: &str, value: String| text.push_str(&format!("  {name:<16} {value}\n"));
    row("target", format!("{} (Web only)", request.app.cli_name()));
    row("account", request.account.as_str().to_owned());
    row("project", request.project.as_str().to_owned());
    row(
        "mapping",
        match mapping {
            Ok(destination) => match agree(request, destination) {
                Ok(()) => format!(
                    "agrees (production branch `{}`)",
                    destination.production_branch.as_str()
                ),
                Err(mismatch) => format!("DISAGREES ({mismatch})"),
            },
            Err(reason) => format!("UNRESOLVED ({reason})"),
        },
    );
    row("branch", request.branch.as_str().to_owned());
    row(
        "environment",
        match request.environment {
            Environment::Production => "production (replaces what the project serves)".to_owned(),
            Environment::Preview => {
                "preview (a PUBLIC *.pages.dev URL; the preview does not isolate its backend)"
                    .to_owned()
            }
        },
    );
    row("artifact", request.artifact.display().to_string());
    row(
        "serves",
        "static Leptos HTML/CSS/JS/WASM only; no Functions, _worker.js, bindings or API".to_owned(),
    );
    text.push_str("  stages\n");
    for (index, stage) in Stage::ALL.iter().enumerate() {
        text.push_str(&format!(
            "    {:>2}. {:<20} {}\n",
            index + 1,
            stage.label(),
            stage.plan_note()
        ));
    }
    for (name, command) in commands {
        text.push_str(&format!("  {name:<9} $ {command}\n"));
    }
    text.push_str(
        "NOT_RUN deploy cloudflare: a plan is not an authenticated readiness check or a deployment.\n",
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: &str = "8339afe44b4e8cd2b633a38a77f1db41";

    fn raw() -> Raw {
        Raw {
            app: HostedApp::CantosTheatre,
            surface: Some(Surface::Web),
            account: ACCOUNT.into(),
            project: "cantos-theatre".into(),
            branch: "main".into(),
            environment: "production".into(),
            artifact: "artifacts/release-build/theatre-web/abc".into(),
            dry_run: true,
        }
    }

    fn destination() -> Destination {
        Destination {
            project: ProjectName::parse("cantos-theatre").unwrap(),
            production_branch: Branch::parse("main").unwrap(),
        }
    }

    #[test]
    fn account_ids_are_exactly_thirty_two_lowercase_hex_characters() {
        assert!(AccountId::parse(ACCOUNT).is_ok());
        for bad in [
            "",
            "8339AFE44B4E8CD2B633A38A77F1DB41",
            "8339afe44b4e8cd2b633a38a77f1db4",
            "8339afe44b4e8cd2b633a38a77f1db411",
            "user@example.com",
            "Manhpdbroad's Account",
        ] {
            assert!(AccountId::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn project_names_refuse_hostnames_uppercase_and_edge_hyphens() {
        assert!(ProjectName::parse("cantos-theatre").is_ok());
        for bad in [
            "",
            "-a",
            "a-",
            "Cantos",
            "cantos.pages.dev",
            "a_b",
            &"a".repeat(59),
        ] {
            assert!(ProjectName::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn branches_refuse_argument_and_revision_syntax() {
        assert!(Branch::parse("main").is_ok());
        assert!(Branch::parse("release/2026-10").is_ok());
        for bad in [
            "",
            "--production",
            "-x",
            "a b",
            "a..b",
            "a//b",
            "/a",
            "a/",
            "a.lock",
            "a~1",
            "a;b",
            "a*",
            ".hidden",
        ] {
            assert!(Branch::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_environment_must_be_named_and_is_never_defaulted() {
        assert_eq!(
            Environment::parse("production"),
            Ok(Environment::Production)
        );
        assert_eq!(Environment::parse("preview"), Ok(Environment::Preview));
        assert!(Environment::parse("prod").is_err());
        assert!(Environment::parse("").is_err());
        assert!(Environment::parse("live").is_err());
    }

    #[test]
    fn a_valid_request_resolves_and_keeps_every_identifier_separate() {
        let request = resolve(&raw()).unwrap();
        assert_eq!(request.app, App::Theatre);
        assert_eq!(request.target(), "theatre");
        assert_eq!(request.component(), "theatre-web");
        assert_eq!(request.account.as_str(), ACCOUNT);
        assert_eq!(request.environment, Environment::Production);
    }

    #[test]
    fn an_invalid_input_refuses_before_anything_else() {
        for mutate in [
            (|r: &mut Raw| r.account = "x".into()) as fn(&mut Raw),
            |r| r.project = "Bad Name".into(),
            |r| r.branch = "--evil".into(),
            |r| r.environment = "live".into(),
            |r| r.artifact = "  ".into(),
        ] {
            let mut input = raw();
            mutate(&mut input);
            assert!(matches!(resolve(&input), Err(Refusal::Usage(_))));
        }
    }

    #[test]
    fn production_must_be_on_the_production_branch_and_preview_must_not_be() {
        let request = resolve(&raw()).unwrap();
        assert_eq!(agree(&request, &destination()), Ok(()));

        let mut feature = raw();
        feature.branch = "feature/x".into();
        let request = resolve(&feature).unwrap();
        assert_eq!(
            agree(&request, &destination()),
            Err(Mismatch::NotTheProductionBranch {
                branch: "feature/x".into(),
                production: "main".into()
            })
        );

        feature.environment = "preview".into();
        assert_eq!(agree(&resolve(&feature).unwrap(), &destination()), Ok(()));

        let mut on_main = raw();
        on_main.environment = "preview".into();
        assert_eq!(
            agree(&resolve(&on_main).unwrap(), &destination()),
            Err(Mismatch::WouldBeProduction {
                branch: "main".into()
            })
        );
    }

    #[test]
    fn a_project_that_is_not_the_mapped_one_is_a_mismatch() {
        let mut other = raw();
        other.project = "cantos-studio".into();
        assert!(matches!(
            agree(&resolve(&other).unwrap(), &destination()),
            Err(Mismatch::Project { .. })
        ));
    }

    #[test]
    fn the_plan_names_every_stage_and_states_that_it_ran_nothing() {
        let request = resolve(&raw()).unwrap();
        let plan = render_plan(
            &request,
            &Ok(destination()),
            &[("deploy", "wrangler pages deploy".to_owned())],
        );
        assert!(plan.contains("nothing was probed, extracted, uploaded or written"));
        assert!(plan.contains("agrees (production branch `main`)"));
        for stage in Stage::ALL {
            assert!(plan.contains(stage.label()), "{}", stage.label());
        }
        assert!(plan.contains("NOT_RUN deploy cloudflare"));
        let unresolved = render_plan(&request, &Err("no mapping".into()), &[]);
        assert!(unresolved.contains("UNRESOLVED (no mapping)"));
    }
}
