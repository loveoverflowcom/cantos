//! Explicit static Web deployment of **one** Leptos app to Cloudflare Pages by Direct Upload
//! (issue #278).
//!
//! The product decision is recorded in
//! [ADR 0043](../../../../docs/decisions/0043-cloudflare-pages-direct-upload.md): Pages Direct
//! Upload was requested; Workers Static Assets is **not** implemented here and no second adapter
//! exists.
//!
//! Unlike `deploy firebase`, this command never builds. It consumes the output of
//! `cargo xtask release-build <app>-web`, verifies it against its manifest, stages it into a new
//! run-owned directory, adds the tracked delivery policy and uploads exactly that directory.
//! A preview deploy publishes a PUBLIC `*.pages.dev` URL; a production deploy replaces what the
//! project serves. `--dry-run` is the way to review either first.
//!
//! The pieces are separated by what they may touch:
//! - [`request`] resolves flags into a validated request, checks it against the operator's mapping
//!   and renders the plan (pure);
//! - [`policy`] validates the tracked `_headers`/`404.html`/`_redirects` policy and the private
//!   account/project mapping (pure);
//! - [`artifact`] verifies the release manifest and extracts the archive safely (pure checks plus
//!   bounded extraction);
//! - [`wrangler`] builds the argv and interprets what Wrangler reports (pure);
//! - [`host`] owns local files, the environment (by name) and the clock;
//! - [`flow`] orders the stages and enforces that nothing follows a failure.
//!
//! # Acceptance status
//!
//! Tested against fixtures and fakes, and exercised once against a real account: on 2026-10-19 a preview
//! deployment of each app passed every stage (see docs/deployment/cloudflare-pages.md). Not accepted,
//! each marked `ACCEPTANCE(NOT_RUN)` (`rg 'ACCEPTANCE\(NOT_RUN\)' xtask docs`):
//!
//! | Not accepted | Needs | Marked in |
//! | --- | --- | --- |
//! | a production deployment of each app and its readback | an explicit go-ahead | [`wrangler`], [`flow`] |
//! | failure shapes other than an invalid token, other Wrangler releases | the same, over time | [`wrangler`] |
//! | Pages' own enforcement of its file-count and per-asset limits | a bundle near the limit | [`flow`] |
//! | custom domains, DNS, browser/API/auth acceptance and cutover | the rollout runbook, separate authority | docs |

mod artifact;
mod flow;
mod host;
mod policy;
mod request;
mod wrangler;

use std::process::ExitCode;

use anyhow::Result;
use clap::Args;

use super::request::HostedApp;
use crate::{capture::SystemCapture, cli::SurfaceArgs, tools::ToolProbe, workspace::Workspace};
use request::{Raw, Request};

const CLOUDFLARE_HELP: &str = "\
Five different things are named here; they are not interchangeable:
  account     the Cloudflare account ID (--account), 32 hex characters
  project     the Pages project name (--project); a Direct Upload project you created
  branch      the Git branch name Pages receives (--branch); production is the project's own branch
  environment `production` or `preview` (--environment), stated, never inferred
  artifact    the verified output of `cargo xtask release-build <app>-web` (--artifact)

Examples:
  # Review the plan. Starts no tool, reads no credential, writes nothing.
  cargo xtask deploy cloudflare cantos-theatre --web --account ACCOUNT_ID --project cantos-theatre \\
    --branch main --environment production --artifact artifacts/release-build/theatre-web/SHA --dry-run

  # Publish a PUBLIC preview of an already-built artifact. A REMOTE WRITE.
  cargo xtask deploy cloudflare cantos-studio --web --account ACCOUNT_ID --project cantos-studio \\
    --branch review-pages --environment preview --artifact artifacts/release-build/studio-web/SHA

Configuration:
  infra/cloudflare/projects.json   your account -> app -> project/production branch mapping (ignored by Git)
  infra/cloudflare/<app>/          tracked headers/404 policy (reviewed in Git)
  infra/cloudflare/wrangler/       pinned Wrangler lockfile; install with `npm ci` there
  CLOUDFLARE_API_TOKEN             Account / Cloudflare Pages / Edit; read by Wrangler, never by xtask
xtask never logs in, creates projects or zones, changes DNS, bindings, billing or the production
branch. Static files only: the API is not served. See docs/deployment/cloudflare-pages.md.";

#[derive(Debug, Args)]
#[command(after_long_help = CLOUDFLARE_HELP)]
pub struct CloudflareArgs {
    /// The Web app to deploy (the API, Forge, mobile and `all` are not targets).
    #[arg(value_enum)]
    pub target: HostedApp,
    #[command(flatten)]
    pub surface: SurfaceArgs,
    /// Cloudflare account ID (required; there is no default or active account).
    #[arg(long, value_name = "ACCOUNT_ID")]
    pub account: String,
    /// Pages project name (required; must be the one mapped to this app).
    #[arg(long, value_name = "PROJECT")]
    pub project: String,
    /// Branch name given to Pages (required; production iff it is the project's production branch).
    #[arg(long, value_name = "BRANCH")]
    pub branch: String,
    /// `production` or `preview` (required; never defaulted).
    #[arg(long, value_name = "ENVIRONMENT")]
    pub environment: String,
    /// The `release-build` output directory of this app (contains manifest.json and the archive).
    #[arg(long, value_name = "DIR")]
    pub artifact: String,
    /// Print the plan only: no tool probe, extraction, login, upload or file write.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn execute(
    workspace: &Workspace,
    args: &CloudflareArgs,
    tools: &dyn ToolProbe,
) -> Result<ExitCode> {
    let raw = Raw {
        app: args.target,
        surface: args.surface.surface(),
        account: args.account.clone(),
        project: args.project.clone(),
        branch: args.branch.clone(),
        environment: args.environment.clone(),
        artifact: args.artifact.clone(),
        dry_run: args.dry_run,
    };
    run_request(workspace, &raw, tools)
}

fn run_request(workspace: &Workspace, raw: &Raw, tools: &dyn ToolProbe) -> Result<ExitCode> {
    let request = match request::resolve(raw) {
        Ok(request) => request,
        Err(refusal) => return Ok(crate::refuse(refusal)),
    };
    if request.dry_run {
        print!("{}", plan_text(workspace, &request));
        return Ok(ExitCode::SUCCESS);
    }
    let mut capture = SystemCapture::new()?;
    let mut host = host::SystemHost::new(workspace);
    let report = flow::run(
        workspace,
        &request,
        flow::Ports {
            tools,
            capture: &mut capture,
            host: &mut host,
        },
    );
    report.print();
    Ok(report.exit_code())
}

/// What the local files say about the destination, without writing anything or starting a tool.
fn local_mapping(workspace: &Workspace, request: &Request) -> Result<request::Destination, String> {
    let text = std::fs::read_to_string(workspace.path(policy::MAPPING_FILE)).ok();
    policy::resolve_mapping(text.as_deref(), &request.account, request.target())
}

/// The reviewable plan. Built from pure values plus the local mapping file only.
fn plan_text(workspace: &Workspace, request: &Request) -> String {
    let run = workspace.path("artifacts/cloudflare/RUN-ID");
    let binary = workspace.path(wrangler::BINARY);
    let inv = wrangler::Invocation {
        binary: &binary,
        cwd: &run,
        account: &request.account,
    };
    let public = run.join("public");
    let output = run.join("out").join("wrangler.ndjson");
    let message = format!("cantos {} COMMIT RUN-ID", request.target());
    let display = |command: crate::command::CommandSpec| command.display(workspace.root());
    request::render_plan(
        request,
        &local_mapping(workspace, request),
        &[
            ("version", display(wrangler::version_command(&inv))),
            ("project", display(wrangler::projects_command(&inv))),
            (
                "publish",
                display(wrangler::deploy_command(
                    &inv,
                    &wrangler::Publish {
                        project: &request.project,
                        branch: &request.branch,
                        directory: &public,
                        commit_hash: "SOURCE-SHA-FROM-MANIFEST",
                        commit_message: &message,
                        output_file: &output,
                    },
                )),
            ),
            (
                "readback",
                display(wrangler::list_command(
                    &inv,
                    &request.project,
                    request.environment,
                )),
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        build::Surface,
        tools::{testing::FakeTools, Tool},
    };

    struct NoProbe;
    impl ToolProbe for NoProbe {
        fn available(&self, tool: Tool) -> bool {
            panic!("a dry run must not probe {tool:?}");
        }
    }

    fn raw() -> Raw {
        Raw {
            app: HostedApp::CantosStudio,
            surface: Some(Surface::Web),
            account: "8339afe44b4e8cd2b633a38a77f1db41".into(),
            project: "cantos-studio".into(),
            branch: "review-pages".into(),
            environment: "preview".into(),
            artifact: "artifacts/release-build/studio-web/SHA".into(),
            dry_run: true,
        }
    }

    #[test]
    fn the_dry_run_probes_nothing_starts_nothing_and_writes_nothing() {
        let workspace = Workspace::discover().unwrap();
        let before = workspace.path("artifacts/cloudflare").exists();
        let code = run_request(&workspace, &raw(), &NoProbe).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(workspace.path("artifacts/cloudflare").exists(), before);
        assert!(!workspace.path("artifacts/cloudflare/RUN-ID").exists());
    }

    #[test]
    fn a_dry_run_works_without_any_tool_and_shows_exact_redacted_argv() {
        let workspace = Workspace::discover().unwrap();
        let _ = FakeTools::without(&Tool::ALL);
        let request = request::resolve(&raw()).unwrap();
        let plan = plan_text(&workspace, &request);
        assert!(plan.contains("pages deploy"), "{plan}");
        assert!(plan.contains("--project-name cantos-studio"));
        assert!(plan.contains("--branch review-pages"));
        assert!(plan.contains("--commit-dirty=false"));
        assert!(plan.contains("CLOUDFLARE_ACCOUNT_ID=8339afe44b4e8cd2b633a38a77f1db41"));
        assert!(plan.contains("pages deployment list"));
        assert!(!plan.contains("$ npx"), "no implicit npx download");
        assert!(
            !plan.to_lowercase().contains("token="),
            "no credential appears in a plan"
        );
    }

    #[test]
    fn an_invalid_request_is_refused_even_in_a_dry_run() {
        let workspace = Workspace::discover().unwrap();
        let mut bad = raw();
        bad.environment = "live".into();
        let code = run_request(&workspace, &bad, &NoProbe).unwrap();
        assert_ne!(code, ExitCode::SUCCESS);
    }
}
