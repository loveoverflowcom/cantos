//! Explicit distribution of real-only development and optimized release Theatre APKs through
//! Firebase App Distribution (#184). Neither channel establishes native runtime acceptance.
//!
//! This is the one deliberate remote write in xtask. Normal `build`, `run`, `verify`,
//! `preflight`, `ci` and `build all` never reach it: it exists only behind
//! `cargo xtask distribute firebase`. A distribution may invite or notify the selected testers,
//! and the plan (`--dry-run`) is the way to review it first.
//!
//! The pieces are small and separated by what they may touch:
//! - [`request`] resolves flags and configuration into a validated plan (pure);
//! - [`apk`] interprets Gradle metadata and the Android tools' reports (pure);
//! - [`firebase`] builds the CLI argv and translates its result into a remote outcome (pure);
//! - [`receipt`] is the per-run record;
//! - [`identity`] derives the version/source identity a development APK must carry (pure);
//! - [`host`] owns local files, Git and the clock;
//! - [`flow`] orders the stages and enforces that nothing follows a failure.

mod apk;
mod firebase;
mod flow;
mod host;
mod identity;
mod receipt;
mod request;

use std::{path::PathBuf, process::ExitCode};

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{capture::SystemCapture, command::Runner, tools::ToolProbe, workspace::Workspace};
use request::{
    AppIdResolution, ArtifactKind, Channel, Configuration, Raw, Target, APP_ID_ENV, CERT_ENV,
};

#[derive(Debug, Args)]
pub struct DistributeArgs {
    #[command(subcommand)]
    pub provider: Provider,
}

#[derive(Debug, Subcommand)]
pub enum Provider {
    /// Build the selected Theatre QA APK and distribute it through Firebase App Distribution.
    Firebase(FirebaseArgs),
}

const FIREBASE_HELP: &str = "Explicit operator distribution only; may notify testers.
  development -> :androidApp:assembleDebug, com.cantos.theatre.dev, debuggable, real-only
  production  -> :androidApp:assembleRelease, com.cantos.theatre, R8/resource shrinking, real-only
  demo/demo-qa are retired and refused. AAB distribution remains unsupported.
Use --api-origin HTTPS_ORIGIN independently of build type; it is the only app configuration input.
Firebase App ID must be registered for the exact package (no .demo/.dev/production substitution).
CANTOS_FIREBASE_DEVELOPMENT_APP_ID / CANTOS_FIREBASE_PRODUCTION_APP_ID or --app-id.
CANTOS_FIREBASE_DEVELOPMENT_CERT_SHA256 / CANTOS_FIREBASE_PRODUCTION_CERT_SHA256 are required.
Release uses only operator-provided CANTOS_RELEASE_KEYSTORE, CANTOS_RELEASE_KEYSTORE_PASSWORD,
CANTOS_RELEASE_KEY_ALIAS and CANTOS_RELEASE_KEY_PASSWORD; never a fallback or generated key.
--dry-run starts no subprocess/network and writes nothing. Exact recipients are mandatory.
No command installs or uninstalls an app. Audit/unsigned bytes are never distributable.";

#[derive(Debug, Args)]
#[command(after_long_help = FIREBASE_HELP)]
pub struct FirebaseArgs {
    /// The application to distribute (Studio is Web-only and is not a target).
    #[arg(value_enum)]
    pub target: Target,
    /// Distribution policy (required; never defaulted).
    #[arg(long, value_enum)]
    pub channel: Channel,
    /// Artifact type; AAB is recognized but unavailable.
    #[arg(long, value_enum, default_value_t = ArtifactKind::Apk)]
    pub artifact: ArtifactKind,
    /// Firebase Android app ID; takes precedence over CANTOS_FIREBASE_DEVELOPMENT_APP_ID.
    #[arg(long, value_name = "ID")]
    pub app_id: Option<String>,
    /// Exact HTTPS API origin embedded in production artifacts; required for production.
    #[arg(long, value_name = "ORIGIN")]
    pub api_origin: Option<String>,
    /// Comma-separated Firebase group aliases (union with --testers).
    #[arg(long, value_delimiter = ',', value_name = "ALIASES")]
    pub groups: Vec<String>,
    /// Comma-separated tester email addresses (union with --groups); never printed.
    #[arg(long, value_delimiter = ',', value_name = "EMAILS")]
    pub testers: Vec<String>,
    /// Release notes text. Mutually exclusive with --release-notes-file.
    #[arg(long, value_name = "TEXT", conflicts_with = "release_notes_file")]
    pub release_notes: Option<String>,
    /// File with UTF-8 release notes. Without either, a version/source summary is generated.
    #[arg(long, value_name = "PATH")]
    pub release_notes_file: Option<PathBuf>,
    /// Print the plan only: no tool probe, build, login, upload or file write.
    #[arg(long)]
    pub dry_run: bool,
}

fn configuration(channel: Channel) -> Configuration {
    let variable = |name: &str| {
        std::env::var_os(name)
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| !value.is_empty())
    };
    Configuration {
        app_id: variable(if channel == Channel::Production {
            request::PRODUCTION_APP_ID_ENV
        } else {
            APP_ID_ENV
        }),
        cert_sha256: variable(match channel {
            Channel::Development | Channel::Demo => CERT_ENV,
            Channel::DemoQa => CERT_ENV,
            Channel::Production => request::PRODUCTION_CERT_ENV,
        }),
    }
}

pub fn execute(
    workspace: &Workspace,
    args: &DistributeArgs,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let Provider::Firebase(args) = &args.provider;
    // Before the tool probe, plan, build, login or upload: no policy is silently remapped (#286).
    if let Some(refusal) = request::retired_lane(args.channel) {
        return Ok(crate::refuse(refusal));
    }
    let raw = Raw {
        api_origin: args.api_origin.clone(),
        target: args.target,
        channel: args.channel,
        artifact: args.artifact,
        app_id: args.app_id.clone(),
        groups: args.groups.clone(),
        testers: args.testers.clone(),
        release_notes: args.release_notes.clone(),
        release_notes_file: args.release_notes_file.clone(),
        dry_run: args.dry_run,
    };
    run_request(workspace, &raw, &configuration(args.channel), tools, runner)
}

fn run_request(
    workspace: &Workspace,
    raw: &Raw,
    configuration: &Configuration,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let request = match request::resolve(raw, configuration, host::read_notes_file) {
        Ok(request) => request,
        Err(refusal) => return Ok(crate::refuse(refusal)),
    };
    if request.dry_run {
        print!("{}", plan_text(workspace, &request));
        return Ok(ExitCode::SUCCESS);
    }
    let live = match request.live() {
        Ok(live) => live,
        Err(refusal) => return Ok(crate::refuse(refusal)),
    };
    let mut capture = SystemCapture::new()?;
    let mut host = host::SystemHost::new(workspace);
    let report = flow::run(
        workspace,
        &live,
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

fn build_command(
    workspace: &Workspace,
    policy: request::Policy,
    origin: Option<&str>,
    identity: Option<&identity::BuildIdentity>,
) -> crate::command::CommandSpec {
    match policy {
        request::Policy::DevelopmentApk => {
            let command =
                crate::theatre_host::gradle_debug_command(workspace, origin.unwrap_or_default());
            match identity {
                Some(identity) => command.args(identity.gradle_args()),
                None => command.args(identity::PLAN_ARGS),
            }
        }
        request::Policy::ProductionApk => {
            crate::command::CommandSpec::new("cargo", workspace.root()).args([
                "xtask",
                "theatre-host",
                "android-release",
                "--operator-signing",
                "--api-origin",
                origin.unwrap_or_default(),
            ])
        }
    }
}

/// The reviewable plan. Built from pure values only: no probe, no file or environment access.
fn plan_text(workspace: &Workspace, request: &request::Request) -> String {
    let gradle = build_command(
        workspace,
        request.policy,
        request.api_origin.as_deref(),
        None,
    )
    .display(workspace.root());
    let placeholder = PathBuf::from("artifacts/distributions/RUN-ID");
    let apk = placeholder.join("artifact/OUTPUT.apk");
    let notes = placeholder.join("release-notes.txt");
    let testers = placeholder.join("testers.txt");
    let app_id = match &request.app_id {
        AppIdResolution::Resolved { id, .. } => id.clone(),
        AppIdResolution::Unresolved => {
            request::AppId::parse("1:0:android:0").expect("the placeholder is a well-formed app ID")
        }
    };
    let mut command = firebase::distribute_command(&firebase::DistributeInputs {
        run_dir: &workspace.path("artifacts/distributions/RUN-ID"),
        staged_apk: &apk,
        app_id: &app_id,
        groups: request.recipients.groups(),
        notes_file: &notes,
        testers_file: (!request.recipients.testers().is_empty()).then_some(testers.as_path()),
    });
    if request.app_id == AppIdResolution::Unresolved {
        for arg in &mut command.args {
            if arg.starts_with("--app=") {
                *arg = "--app=UNRESOLVED".to_owned();
            }
        }
    }
    request::render_plan(request, &gradle, &command.display(workspace.root()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile_toolchain;
    use crate::{
        command::testing::RecordingRunner,
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
            api_origin: Some("https://api.cantos.example".into()),
            target: Target::CantosTheatre,
            channel: Channel::Development,
            artifact: ArtifactKind::Apk,
            app_id: None,
            groups: vec!["cantos-internal".into()],
            testers: vec!["private.person@example.com".into()],
            release_notes: None,
            release_notes_file: None,
            dry_run: true,
        }
    }

    #[test]
    fn the_dry_run_plan_starts_nothing_and_works_without_configuration() {
        let workspace = Workspace::discover().unwrap();
        let mut runner = RecordingRunner::default();
        let code = run_request(
            &workspace,
            &raw(),
            &Configuration::default(),
            &NoProbe,
            &mut runner,
        )
        .unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(runner.commands.is_empty());
        let request =
            request::resolve(&raw(), &Configuration::default(), |_| Ok(Vec::new())).unwrap();
        let plan = plan_text(&workspace, &request);
        assert!(plan.contains("UNRESOLVED"));
        assert!(plan.contains("--app=UNRESOLVED"));
        assert!(plan.contains("--groups=cantos-internal"));
        assert!(plan.contains("--testers-file=artifacts/distributions/RUN-ID/testers.txt"));
        assert!(plan.contains(":androidApp:assembleDebug"));
        assert!(plan.contains("--no-daemon"));
        assert!(!plan.contains("private.person"), "{plan}");
    }

    #[test]
    fn plans_for_both_channels_name_the_api_origin_and_no_provider_identity_input() {
        let workspace = Workspace::discover().unwrap();
        for channel in [Channel::Development, Channel::Production] {
            let raw = Raw { channel, ..raw() };
            let request =
                request::resolve(&raw, &Configuration::default(), |_| Ok(Vec::new())).unwrap();
            let plan = plan_text(&workspace, &request).to_ascii_lowercase();
            assert!(plan.contains("https://api.cantos.example"), "{plan}");
            assert!(
                !plan.contains("issuer") && !plan.contains("callback"),
                "{plan}"
            );
        }
    }

    #[test]
    fn a_resolved_plan_names_the_app_and_its_source() {
        let workspace = Workspace::discover().unwrap();
        let configuration = Configuration {
            app_id: Some("1:123:android:abc".into()),
            ..Configuration::default()
        };
        let request = request::resolve(&raw(), &configuration, |_| Ok(Vec::new())).unwrap();
        let plan = plan_text(&workspace, &request);
        assert!(
            plan.contains(&format!("1:123:android:abc (from {APP_ID_ENV})")),
            "{plan}"
        );
        assert!(plan.contains("--app=1:123:android:abc"));
    }

    #[test]
    fn an_unavailable_combination_runs_no_tool_and_no_runner() {
        let workspace = Workspace::discover().unwrap();
        let mut runner = RecordingRunner::default();
        for (channel, artifact) in [
            (Channel::Development, ArtifactKind::Aab),
            (Channel::Production, ArtifactKind::Aab),
            (Channel::Production, ArtifactKind::Aab),
        ] {
            let raw = Raw {
                channel,
                artifact,
                dry_run: false,
                ..raw()
            };
            let code = run_request(
                &workspace,
                &raw,
                &Configuration::default(),
                &NoProbe,
                &mut runner,
            )
            .unwrap();
            assert_eq!(
                code,
                ExitCode::FAILURE,
                "UNSUPPORTED is a failing, reported status"
            );
        }
        assert!(runner.commands.is_empty());
    }

    #[test]
    fn a_live_run_without_an_app_id_is_a_usage_error_before_any_tool() {
        let workspace = Workspace::discover().unwrap();
        let mut runner = RecordingRunner::default();
        let raw = Raw {
            dry_run: false,
            ..raw()
        };
        let code = run_request(
            &workspace,
            &raw,
            &Configuration::default(),
            &NoProbe,
            &mut runner,
        )
        .unwrap();
        assert_eq!(code, ExitCode::from(2));
        assert!(runner.commands.is_empty());
    }

    #[test]
    fn oversized_or_non_regular_notes_stop_live_and_dry_requests_before_tools() {
        let workspace = Workspace::discover().unwrap();
        let root = std::env::temp_dir().join(format!(
            "xtask-distribute-notes-request-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let oversized = root.join("notes.txt");
        std::fs::write(&oversized, vec![b'x'; request::NOTES_LIMIT * 4]).unwrap();
        for path in [&oversized, &root] {
            for dry_run in [true, false] {
                let mut runner = RecordingRunner::default();
                let raw = Raw {
                    app_id: Some("1:123:android:abc".into()),
                    release_notes_file: Some(path.clone()),
                    dry_run,
                    ..raw()
                };
                let code = run_request(
                    &workspace,
                    &raw,
                    &Configuration::default(),
                    &NoProbe,
                    &mut runner,
                )
                .unwrap();
                assert_eq!(code, ExitCode::from(2));
                assert!(runner.commands.is_empty());
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_requests_are_usage_errors_before_any_tool() {
        let workspace = Workspace::discover().unwrap();
        let mut runner = RecordingRunner::default();
        for raw in [
            Raw {
                groups: vec![],
                testers: vec![],
                ..raw()
            },
            Raw {
                groups: vec![String::new()],
                ..raw()
            },
            Raw {
                release_notes: Some("a".into()),
                release_notes_file: Some("n.txt".into()),
                ..raw()
            },
            Raw {
                app_id: Some("com.cantos.theatre.dev".into()),
                ..raw()
            },
        ] {
            let code = run_request(
                &workspace,
                &raw,
                &Configuration::default(),
                &NoProbe,
                &mut runner,
            )
            .unwrap();
            assert_eq!(code, ExitCode::from(2));
        }
        assert!(runner.commands.is_empty());
    }

    #[test]
    fn missing_tools_never_reach_the_dry_run_plan() {
        // FakeTools without anything installed would fail a live run, but a plan needs none.
        let workspace = Workspace::discover().unwrap();
        let mut runner = RecordingRunner::default();
        let nothing = FakeTools::without(&Tool::ALL);
        let code = run_request(
            &workspace,
            &raw(),
            &Configuration::default(),
            &nothing,
            &mut runner,
        )
        .unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    /// Host-capable check of the local half only: the installed Android tools against a
    /// real, freshly built demo APK. It starts no Firebase process and makes no remote request.
    /// Run: `cargo xtask theatre-host android-debug --api-origin ORIGIN` then
    /// `cargo test -p xtask --locked -- --ignored --nocapture installed_android_tools`.
    #[test]
    #[ignore = "needs a built demo APK and an installed Android SDK"]
    fn installed_android_tools_accept_the_built_demo_apk() {
        accept_built_apk(request::Policy::DevelopmentApk);
    }

    /// The same check for the optimized QA output (`:androidApp:assembleRelease`, operator-signed).
    #[test]
    #[ignore = "needs a built QA APK and an installed Android SDK"]
    fn installed_android_tools_accept_the_built_qa_apk() {
        accept_built_apk(request::Policy::ProductionApk);
    }

    fn accept_built_apk(policy: request::Policy) {
        use crate::capture::{Capture, SystemCapture};
        use apk::{
            badging_command, parse_badging, parse_output_metadata, parse_signing, signer_command,
            validate, ApkFacts,
        };
        use host::Host;

        let workspace = Workspace::discover().unwrap();
        let mut host = host::SystemHost::new(&workspace);
        let sdk = host
            .android_sdk()
            .expect("ANDROID_HOME or ANDROID_SDK_ROOT");
        let build_tools = sdk.join(mobile_toolchain::ANDROID_BUILD_TOOLS);
        let metadata = host.read_output_metadata(policy).expect("a built variant");
        let declared = parse_output_metadata(&metadata, policy).unwrap();
        let run = host.create_run().unwrap();
        let staged = host
            .stage_output(policy, &run, &declared.file_name)
            .unwrap();
        let mut capture = SystemCapture::new().unwrap();
        let timeout = std::time::Duration::from_secs(120);
        let badging = capture
            .capture(
                &badging_command(&build_tools.join("aapt2"), &staged.path, &run.path),
                timeout,
            )
            .unwrap();
        let signer = capture
            .capture(
                &signer_command(&build_tools.join("apksigner"), &staged.path, &run.path),
                timeout,
            )
            .unwrap();
        let facts = ApkFacts {
            sha256: staged.sha256.clone(),
            size_bytes: staged.size_bytes,
            badging: parse_badging(badging.code, &badging.stdout, &badging.stderr).unwrap(),
            signing: parse_signing(signer.code, &signer.stdout, &signer.stderr).unwrap(),
        };
        let findings = validate(policy, &declared, &facts, None);
        println!(
            "package={} versionName={} versionCode={} debuggable={} size={} sha256={} signer_cert_sha256={} schemes(v1,v2,v3)=({},{},{})",
            facts.badging.package,
            facts.badging.version_name,
            facts.badging.version_code,
            facts.badging.debuggable,
            facts.size_bytes,
            facts.sha256,
            facts.signing.cert_sha256,
            facts.signing.v1,
            facts.signing.v2,
            facts.signing.v3
        );
        let rehashed = host.hash_file(&staged.path).unwrap();
        std::fs::remove_dir_all(&run.path).unwrap();
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(rehashed.0, staged.sha256);
    }

    #[test]
    fn no_ordinary_gate_build_or_ci_plan_can_reach_distribution() {
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
            crate::build::BuildTarget::All,
            crate::build::BuildTarget::CantosServer,
            crate::build::BuildTarget::CantosIdentity,
            crate::build::BuildTarget::CantosForge,
        ] {
            if let Ok(steps) =
                crate::build::plan(&workspace, target, None, crate::build::Profile::Dev)
            {
                commands.extend(steps.into_iter().flat_map(|step| step.commands));
            }
        }
        assert!(!commands.is_empty());
        for command in commands {
            let line = format!("{} {}", command.program, command.args.join(" "));
            assert!(
                !line.contains("firebase") && !line.contains("distribute"),
                "an ordinary plan reaches distribution: {line}"
            );
        }
    }
}
