//! Explicit static Web deployment of **one** Leptos app to Firebase Hosting (issue #199).
//!
//! This is the second deliberate remote write in xtask, beside `distribute firebase`. Normal
//! `build`, `run`, `verify`, `preflight`, `ci` and `build all` never reach it: it exists only behind
//! `cargo xtask deploy firebase`. A *preview* deploy publishes a PUBLIC, expiring URL; a *live*
//! deploy replaces what a site serves. `--dry-run` is the way to review either first.
//!
//! The command serves the static HTML/CSS/JS/WASM bundle and nothing else: the Rust API,
//! PostgreSQL, Kanidm and object storage stay outside Firebase Hosting, and no Firebase SDK,
//! Cloud Functions or Cloud Run proxy is added. It runs no GitHub Action and is never triggered by
//! a commit or a push.
//!
//! The pieces are small and separated by what they may touch:
//! - [`request`] resolves flags into a validated request and renders the plan (pure);
//! - [`hosting_config`] validates the tracked Hosting policy, resolves the operator's site mapping
//!   and renders the run-specific effective configuration (pure);
//! - [`bundle`] decides from a scan whether the built bytes are a deployable bundle of the selected
//!   app and fingerprints them (pure);
//! - [`firebase`] builds the CLI argv and translates its output into a remote outcome (pure);
//! - [`smoke`] judges read-only HTTPS observations of the released URL (pure);
//! - [`receipt`] is the per-run record;
//! - [`host`] owns local files, Git and the clock;
//! - [`flow`] orders the stages and enforces that nothing follows a failure.
//!
//! # Acceptance status
//!
//! Everything below is implemented and tested against fixtures, fakes and local files. What could
//! **not** be accepted in this change is marked at its place in the code with
//! `ACCEPTANCE(NOT_RUN)` (`rg 'ACCEPTANCE\(NOT_RUN\)' xtask services docs`), and is gathered in
//! [docs/evidence/issue199-firebase-hosting.md](../../../docs/evidence/issue199-firebase-hosting.md):
//!
//! | Not accepted | Needs | Marked in |
//! | --- | --- | --- |
//! | a real preview deploy and a real live deploy of each app, and that the unselected site is unchanged | an operator-owned Firebase project with two Hosting sites and an explicit go-ahead | [`flow`], [`firebase`] |
//! | the Firebase CLI response shapes (captured, not reconstructed from sources) | the same run | [`firebase`] |
//! | HTTP smoke against real Hosting (media types, `Cache-Control`, propagation delay) | the same run | [`smoke`] |
//! | Hosting's own channel-ID, file-count and size limits | the same run | [`request`], [`bundle`] |
//! | the API origin and authentication contract (sign-in, catalog, library, playback) | a reviewed origin/auth decision and client work; this command cannot supply it | [`flow`] (`outcome_notes`), docs |
//! | long-lived caching of hashed assets | an oracle that a URL's bytes change whenever its name must | [`hosting_config`] |
//! | rollback or promotion automation | deliberately out of scope; the console's rollback is documented | docs |
//!
//! `firebase-tools` **15.34.0** is the pinned contract; the machine this was built on had 15.33.0,
//! so the live path there stops at the `Firebase CLI` stage (`UNSUPPORTED`) by design.

mod bundle;
mod cloudflare;
mod firebase;
mod flow;
mod host;
mod hosting_config;
mod receipt;
mod request;
mod smoke;
mod vps;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{
    build, capture::SystemCapture, cli::SurfaceArgs, command::Runner, tools::ToolProbe,
    workspace::Workspace,
};
use hosting_config::{resolve_mapping, Policy, Resolution};
use request::{Channel, HostedApp, Raw, Request};

#[derive(Debug, Args)]
pub struct DeployArgs {
    #[command(subcommand)]
    pub provider: Provider,
}

#[derive(Debug, Subcommand)]
pub enum Provider {
    /// Build one Leptos Web app once and deploy it to Firebase Hosting (preview or live).
    Firebase(FirebaseArgs),
    /// Publish a verified release-build Web artifact to Cloudflare Pages (Direct Upload).
    Cloudflare(cloudflare::CloudflareArgs),
    /// Build the API image locally and deploy it to the production VPS over SSH.
    Vps(vps::VpsArgs),
}

const FIREBASE_HELP: &str = "\
Four different things are named here; they are not interchangeable:
  project   the Firebase project ID (--project)
  target    the local deploy target: cantos-theatre -> theatre, cantos-studio -> studio
  site      the Hosting site ID the target means; read from your own .firebaserc
  channel   `live`, or the ID of a PUBLIC preview channel

Examples:
  # Review the plan. Starts no tool, reads no credential, writes nothing.
  cargo xtask deploy firebase cantos-theatre --web --project PROJECT_ID --channel review-hosting --dry-run

  # Build Theatre Web once, validate its bundle, publish a PUBLIC preview. A REMOTE WRITE.
  cargo xtask deploy firebase cantos-theatre --web --project PROJECT_ID --channel review-hosting --expires 7d

  # Replace what the Theatre site serves. A deliberate LIVE release.
  cargo xtask deploy firebase cantos-theatre --web --project PROJECT_ID --channel live

  # Studio is independent: it never builds or deploys Theatre.
  cargo xtask deploy firebase cantos-studio --web --project PROJECT_ID --channel live

Configuration:
  CANTOS_API_ORIGIN                     required exact HTTPS API origin for an actual deploy
  infra/firebase/hosting/firebase.json   tracked policy (reviewed in Git)
  infra/firebase/hosting/.firebaserc     your target -> site mapping (ignored by Git)
The Firebase CLI authenticates by its own login or Application Default Credentials; xtask never
reads, prints or stores a credential and never logs in. Static files only: the API is not served.
See docs/deployment/firebase-hosting.md.";

#[derive(Debug, Args)]
#[command(after_long_help = FIREBASE_HELP)]
pub struct FirebaseArgs {
    /// The Web app to deploy (the API, Forge, mobile and `all` are not targets).
    #[arg(value_enum)]
    pub target: HostedApp,
    #[command(flatten)]
    pub surface: SurfaceArgs,
    /// Firebase project ID (required; there is no default or active project).
    #[arg(long, value_name = "PROJECT_ID")]
    pub project: String,
    /// `live` for a deliberate live release, or a preview channel ID (required; never defaulted).
    #[arg(long, value_name = "CHANNEL")]
    pub channel: String,
    /// Preview channel lifetime such as 30m, 12h or 7d (at most 30d; CLI default 7d). Not for live.
    #[arg(long, value_name = "DURATION")]
    pub expires: Option<String>,
    /// Print the plan only: no tool probe, build, login, upload or file write.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn execute(
    workspace: &Workspace,
    args: &DeployArgs,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let args = match &args.provider {
        Provider::Firebase(args) => args,
        Provider::Cloudflare(args) => return cloudflare::execute(workspace, args, tools),
        Provider::Vps(args) => return vps::execute(workspace, args, runner),
    };
    let raw = Raw {
        app: args.target,
        surface: args.surface.surface(),
        project: args.project.clone(),
        channel: args.channel.clone(),
        expires: args.expires.clone(),
        dry_run: args.dry_run,
    };
    run_request(workspace, &raw, tools, runner)
}

fn run_request(
    workspace: &Workspace,
    raw: &Raw,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let request = match request::resolve(raw) {
        Ok(request) => request,
        Err(refusal) => return Ok(crate::refuse(refusal)),
    };
    if request.dry_run {
        let resolution = local_resolution(workspace, &request);
        print!("{}", plan_text(workspace, &request, &resolution));
        return Ok(ExitCode::SUCCESS);
    }
    let mut capture = SystemCapture::new()?;
    let mut host = host::SystemHost::new(workspace);
    let report = flow::run(
        workspace,
        &request,
        flow::Ports {
            tools,
            runner,
            capture: &mut capture,
            host: &mut host,
        },
    );
    report.print();
    Ok(report.exit_code())
}

/// What the local files say about the destination, without writing anything or starting a tool.
fn local_resolution(workspace: &Workspace, request: &Request) -> Resolution {
    let files = match host::read_config_files(workspace) {
        Ok(files) => files,
        Err(why) => return Resolution::Unresolved { reason: why },
    };
    if let Err(error) = Policy::parse(&files.policy) {
        return Resolution::Unresolved {
            reason: error.to_string(),
        };
    }
    Resolution::from_mapping(resolve_mapping(
        files.firebaserc.as_deref(),
        &request.project,
        request.target(),
    ))
}

/// The reviewable plan. Built from pure values only: no probe, no file or environment access.
fn plan_text(workspace: &Workspace, request: &Request, resolution: &Resolution) -> String {
    let run = workspace.path("artifacts/hosting/RUN-ID");
    let config = run.join("firebase.json");
    let build_command = build::release_bundle(
        workspace,
        request.app,
        &run.join(hosting_config::PUBLIC_DIR),
    )
    .display(workspace.root());
    let inv = firebase::Invocation {
        project: &request.project,
        config: &config,
        cwd: &run,
    };
    let message = format!("cantos {} COMMIT RUN-ID", request.app.cli_name());
    let kind = match &request.channel {
        Channel::Preview(channel) => firebase::ReleaseKind::Preview {
            channel: channel.as_str(),
            target: request.target(),
            expires: request.expires.as_ref().map(|expiry| expiry.as_arg()),
        },
        Channel::Live => firebase::ReleaseKind::Live {
            target: request.target(),
            message: &message,
        },
    };
    let display = |command: crate::command::CommandSpec| command.display(workspace.root());
    request::render_plan(
        request,
        resolution,
        &build_command,
        &[
            ("lookup", display(firebase::sites_command(&inv))),
            ("release", display(firebase::release_command(&inv, kind))),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::{Cli, Task},
        command::testing::RecordingRunner,
        config::App,
        tools::{testing::FakeTools, Tool},
    };
    use clap::Parser;

    struct NoProbe;
    impl ToolProbe for NoProbe {
        fn available(&self, tool: Tool) -> bool {
            panic!("a dry run must not probe {tool:?}");
        }
    }

    fn raw() -> Raw {
        Raw {
            app: HostedApp::CantosTheatre,
            surface: Some(build::Surface::Web),
            project: "cantos-example".into(),
            channel: "review-hosting".into(),
            expires: None,
            dry_run: true,
        }
    }

    fn unresolved() -> Resolution {
        Resolution::Unresolved {
            reason: "no .firebaserc".into(),
        }
    }

    fn plan_for(raw: &Raw) -> String {
        let workspace = Workspace::discover().unwrap();
        let request = request::resolve(raw).unwrap();
        plan_text(&workspace, &request, &unresolved())
    }

    #[test]
    fn the_dry_run_plan_starts_nothing_probes_nothing_and_writes_nothing() {
        let workspace = Workspace::discover().unwrap();
        let mut runner = RecordingRunner::default();
        let code = run_request(&workspace, &raw(), &NoProbe, &mut runner).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(runner.commands.is_empty());
        assert!(
            !workspace.path("artifacts/hosting/RUN-ID").exists(),
            "a dry run must not create the placeholder run directory"
        );
    }

    #[test]
    fn relocated_config_and_staging_keep_spaces_and_ignore_legacy_project_authority() {
        let root = std::env::temp_dir().join(format!(
            "cantos hosting root with spaces {}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join(hosting_config::CONFIG_DIR)).unwrap();
        std::fs::create_dir_all(root.join("services/firebase-hosting")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        std::fs::write(
            root.join(hosting_config::CONFIG_DIR).join("firebase.json"),
            include_str!("../../../infra/firebase/hosting/firebase.json"),
        )
        .unwrap();
        std::fs::write(
            root.join(hosting_config::CONFIG_DIR).join(".firebaserc"),
            r#"{"targets":{"cantos-example":{"hosting":{"theatre":["selected-site"]}}}}"#,
        )
        .unwrap();
        std::fs::write(root.join(".firebaserc"), "invalid legacy root authority").unwrap();
        std::fs::write(
            root.join("services/firebase-hosting/firebase.json"),
            "invalid legacy policy",
        )
        .unwrap();
        let workspace = Workspace::fixture(&root).unwrap();
        let request = request::resolve(&raw()).unwrap();
        let resolution = local_resolution(&workspace, &request);
        assert_eq!(
            resolution,
            Resolution::Resolved {
                site: hosting_config::SiteId::parse("selected-site").unwrap()
            }
        );
        let mut runner = RecordingRunner::default();
        assert_eq!(
            run_request(&workspace, &raw(), &NoProbe, &mut runner).unwrap(),
            ExitCode::SUCCESS
        );
        assert!(runner.commands.is_empty());
        assert!(
            !root.join("artifacts").exists(),
            "planning writes no staging directory"
        );
        let run = root.join("artifacts/hosting/RUN-ID");
        let config = run.join("firebase.json");
        let command = firebase::release_command(
            &firebase::Invocation {
                project: &request.project,
                config: &config,
                cwd: &run,
            },
            firebase::ReleaseKind::Preview {
                channel: "review-hosting",
                target: "theatre",
                expires: None,
            },
        );
        assert_eq!(command.cwd, run);
        assert_eq!(
            command
                .args
                .windows(2)
                .find(|pair| pair[0] == "--config")
                .unwrap()[1],
            config.to_string_lossy()
        );
        assert_eq!(command.to_command().get_current_dir(), Some(run.as_path()));
        assert_eq!(
            command
                .to_command()
                .get_args()
                .filter(|arg| *arg == config.as_os_str())
                .count(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_dry_run_works_without_firebase_trunk_credentials_or_any_tool() {
        let workspace = Workspace::discover().unwrap();
        let mut runner = RecordingRunner::default();
        let nothing = FakeTools::without(&Tool::ALL);
        let code = run_request(&workspace, &raw(), &nothing, &mut runner).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(runner.commands.is_empty());
    }

    #[test]
    fn the_preview_plan_shows_the_exact_redacted_argv_and_the_unresolved_site() {
        let plan = plan_for(&raw());
        assert!(plan.contains("UNRESOLVED (no .firebaserc)"));
        assert!(
            plan.contains("hosting:channel:deploy review-hosting"),
            "{plan}"
        );
        assert!(plan.contains("--only theatre"));
        assert!(plan.contains("--project cantos-example"));
        assert!(plan.contains("--no-authorized-domains --non-interactive --json"));
        assert!(plan.contains("artifacts/hosting/RUN-ID/firebase.json"));
        assert!(plan.contains("hosting:sites:list"));
        assert!(plan.contains("trunk build --locked --release"));
        assert!(
            plan.contains("--dist artifacts/hosting/RUN-ID/dist") || plan.contains("RUN-ID/dist")
        );
        assert!(plan.contains("--public-url /"));
        assert!(
            !plan.contains("--expires"),
            "no expiry unless asked: {plan}"
        );
        assert!(plan.contains("PUBLIC preview channel"));
        assert!(plan.contains("NOT_RUN deploy firebase"));
    }

    #[test]
    fn the_preview_plan_passes_expiry_through_and_the_live_plan_uses_the_deploy_command() {
        let expiring = plan_for(&Raw {
            expires: Some("12h".into()),
            ..raw()
        });
        assert!(expiring.contains("--expires 12h"), "{expiring}");
        let live = plan_for(&Raw {
            channel: "live".into(),
            ..raw()
        });
        assert!(live.contains("firebase deploy"), "{live}");
        assert!(live.contains("--only hosting:theatre"));
        assert!(live.contains("LIVE release"));
        assert!(!live.contains("hosting:channel:deploy"));
        assert!(!live.contains("--no-authorized-domains"));
        let studio = plan_for(&Raw {
            app: HostedApp::CantosStudio,
            channel: "live".into(),
            ..raw()
        });
        assert!(studio.contains("--only hosting:studio"));
        assert!(studio.contains("apps/cantos-studio/web"));
        assert!(!studio.contains("theatre"), "{studio}");
    }

    #[test]
    fn a_resolved_plan_names_the_site_and_where_it_came_from() {
        let workspace = Workspace::discover().unwrap();
        let request = request::resolve(&raw()).unwrap();
        let resolution = Resolution::Resolved {
            site: hosting_config::SiteId::parse("example-theatre").unwrap(),
        };
        let plan = plan_text(&workspace, &request, &resolution);
        assert!(
            plan.contains("example-theatre (from .firebaserc)"),
            "{plan}"
        );
    }

    #[test]
    fn a_missing_mapping_reads_as_unresolved_not_as_a_guess() {
        let workspace = Workspace::discover().unwrap();
        let request = request::resolve(&raw()).unwrap();
        // A clean checkout has the tracked policy and no operator mapping; a developer's own
        // mapping for another project must not resolve either.
        match local_resolution(&workspace, &request) {
            Resolution::Unresolved { reason } => assert!(
                reason.contains("firebaserc") || reason.contains("project"),
                "{reason}"
            ),
            Resolution::Resolved { .. } => {
                // Only possible if the operator mapped exactly this project ID locally.
            }
        }
    }

    #[test]
    fn unsupported_and_malformed_requests_run_no_tool_and_no_runner() {
        let workspace = Workspace::discover().unwrap();
        let mut runner = RecordingRunner::default();
        for (raw, code) in [
            (
                Raw {
                    surface: Some(build::Surface::Mobile),
                    dry_run: false,
                    ..raw()
                },
                ExitCode::FAILURE,
            ),
            (
                Raw {
                    surface: None,
                    dry_run: false,
                    ..raw()
                },
                ExitCode::from(2),
            ),
            (
                Raw {
                    app: HostedApp::CantosStudio,
                    surface: Some(build::Surface::Mobile),
                    dry_run: false,
                    ..raw()
                },
                ExitCode::from(2),
            ),
            (
                Raw {
                    channel: "Live".into(),
                    dry_run: false,
                    ..raw()
                },
                ExitCode::from(2),
            ),
            (
                Raw {
                    channel: "live".into(),
                    expires: Some("7d".into()),
                    dry_run: false,
                    ..raw()
                },
                ExitCode::from(2),
            ),
            (
                Raw {
                    project: "Not A Project".into(),
                    dry_run: true,
                    ..raw()
                },
                ExitCode::from(2),
            ),
        ] {
            let result = run_request(&workspace, &raw, &NoProbe, &mut runner).unwrap();
            assert_eq!(result, code, "{raw:?}");
        }
        assert!(runner.commands.is_empty());
    }

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        let mut argv = vec!["cargo xtask", "deploy", "firebase"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv)
    }

    #[test]
    fn the_command_line_requires_app_project_and_channel_and_rejects_other_targets() {
        let base = [
            "cantos-theatre",
            "--web",
            "--project",
            "cantos-example",
            "--channel",
            "live",
        ];
        match parse(&base).unwrap().command {
            Task::Deploy(args) => {
                let Provider::Firebase(args) = args.provider else {
                    panic!("not a firebase deploy");
                };
                assert_eq!(args.target.app(), App::Theatre);
                assert!(args.surface.web && !args.surface.mobile);
                assert_eq!(
                    (args.project.as_str(), args.channel.as_str()),
                    ("cantos-example", "live")
                );
                assert_eq!(args.expires, None);
                assert!(!args.dry_run);
            }
            other => panic!("{other:?}"),
        }
        // Each of the three explicit inputs is required.
        for missing in [
            &["cantos-theatre", "--web", "--channel", "live"][..],
            &["cantos-theatre", "--web", "--project", "cantos-example"],
            &["--web", "--project", "cantos-example", "--channel", "live"],
        ] {
            assert!(parse(missing).is_err(), "{missing:?}");
        }
        // Only the two Web apps are targets: not the API, Forge, `all`, or Theatre's alias.
        for target in [
            "cantos-api",
            "cantos-forge",
            "all",
            "cantos",
            "services",
            "android",
        ] {
            assert!(
                parse(&[
                    target,
                    "--web",
                    "--project",
                    "cantos-example",
                    "--channel",
                    "live"
                ])
                .is_err(),
                "{target}"
            );
        }
        // `--web` and `--mobile` are mutually exclusive; `--mobile` parses and is refused later.
        assert!(parse(&[
            "cantos-theatre",
            "--web",
            "--mobile",
            "--project",
            "p",
            "--channel",
            "x"
        ])
        .is_err());
        assert!(parse(&[
            "cantos-theatre",
            "--mobile",
            "--project",
            "cantos-example",
            "--channel",
            "x"
        ])
        .is_ok());
        // There is no way to select a deploy-all, an artifact path or a no-build mode.
        for flag in [
            "--all",
            "--no-build",
            "--artifact",
            "--public",
            "--site",
            "--only",
        ] {
            assert!(
                parse(&[
                    "cantos-theatre",
                    "--web",
                    "--project",
                    "cantos-example",
                    "--channel",
                    "x",
                    flag,
                    "y"
                ])
                .is_err(),
                "{flag}"
            );
        }
    }

    /// Host-capable check of the local half only: the real release build of each app through the
    /// shared Trunk command, the real directory scan and the real validator. It starts no Firebase
    /// process and makes no remote request.
    /// Run: `cargo test -p xtask --locked -- --ignored --nocapture real_release_bundles_validate`.
    #[test]
    #[ignore = "needs Trunk, the wasm32 target and a few minutes for two release builds"]
    fn real_release_bundles_validate() {
        use crate::command::Runner;

        let workspace = Workspace::discover().unwrap();
        let root =
            std::env::temp_dir().join(format!("xtask-deploy-real-bundles-{}", std::process::id()));
        let mut runner = crate::command::SystemRunner::new(workspace.root());
        let mut built = Vec::new();
        for app in App::ALL {
            let dist = root.join(app.cli_name()).join(hosting_config::PUBLIC_DIR);
            runner
                .run(&build::release_bundle(&workspace, app, &dist))
                .expect("the release build");
            let scan = host::scan_directory(&dist).unwrap();
            let manifest =
                bundle::validate(app, &scan).unwrap_or_else(|findings| panic!("{findings:?}"));
            let probes = smoke::probes(&manifest, "RUN");
            println!(
                "{}: files={} bytes={} manifest_sha256={} entry={:?} probes={}",
                app.cli_name(),
                manifest.files.len(),
                manifest.total_bytes,
                manifest.digest,
                manifest.entry_assets,
                probes.len()
            );
            assert!(manifest
                .entry_assets
                .iter()
                .any(|a| a.ends_with("_bg.wasm")));
            built.push((app, dist, scan));
        }
        // Each real bundle is refused as the other app, so a stale or mixed output cannot ship.
        for (app, _, scan) in &built {
            let other = if *app == App::Theatre {
                App::Studio
            } else {
                App::Theatre
            };
            assert!(
                bundle::validate(other, scan).is_err(),
                "{app:?} accepted as {other:?}"
            );
        }
        // One appended byte in a real script no longer matches the integrity the real page pins.
        let (app, dist, _) = &built[0];
        let script = std::fs::read_dir(dist)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|e| e == "js"))
            .expect("a script");
        let mut bytes = std::fs::read(&script).unwrap();
        bytes.push(b'\n');
        std::fs::write(&script, bytes).unwrap();
        let findings = bundle::validate(*app, &host::scan_directory(dist).unwrap()).unwrap_err();
        assert!(
            findings
                .iter()
                .any(|f| matches!(f, bundle::Finding::IntegrityMismatch(_))),
            "{findings:?}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_ordinary_gate_build_or_ci_plan_can_reach_deployment() {
        let workspace = Workspace::discover().unwrap();
        let mut commands = Vec::new();
        let mut gates = crate::gate::full();
        gates.push(crate::gate::Gate::ContractTests);
        for gate in gates {
            commands.extend(gate.commands(&workspace));
        }
        commands.extend(
            crate::ci::steps(&workspace, &crate::ci::CiJob::ALL)
                .into_iter()
                .flat_map(|step| step.commands),
        );
        for target in [
            build::BuildTarget::All,
            build::BuildTarget::CantosApi,
            build::BuildTarget::CantosForge,
            build::BuildTarget::CantosStudio,
            build::BuildTarget::CantosTheatre,
        ] {
            for surface in [None, Some(build::Surface::Web)] {
                for profile in [build::Profile::Dev, build::Profile::Release] {
                    if let Ok(steps) = build::plan(&workspace, target, surface, profile, false) {
                        commands.extend(steps.into_iter().flat_map(|step| step.commands));
                    }
                }
            }
        }
        assert!(!commands.is_empty());
        for command in commands {
            let line = format!("{} {}", command.program, command.args.join(" "));
            assert!(
                !line.contains("firebase") && !line.contains("hosting") && !line.contains("deploy"),
                "an ordinary plan reaches deployment: {line}"
            );
        }
    }
}
