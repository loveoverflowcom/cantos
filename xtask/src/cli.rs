//! The command-line contract. Keep it small: every command maps to an implemented capability,
//! and anything without infrastructure is refused as UNSUPPORTED rather than simulated.

use clap::{Args, Parser, Subcommand};

use crate::{
    build::{BuildTarget, Surface},
    ci::CiJob,
    compose_preview::ComposePreviewArgs,
    config::DEFAULT_BASE,
    gate::GateKind,
    kani::Kernel,
    preflight::PreflightProfile,
    retired::{Retired, Retirement},
    run::{AppTarget, LocalMail, RunTarget},
    seed::{SeedOptions, SeedTarget},
    theatre_host::HostAction,
    theatre_preview::TheatrePreviewAction,
    ui_preview::{PreviewAction, Renderer},
};

#[derive(Debug, Parser)]
#[command(
    name = "cargo xtask",
    bin_name = "cargo xtask",
    about = "Build, run and verify the Cantos repository through one command surface",
    after_help = "Repository-level operations go through cargo xtask; scripts/ and external \
                  tools are its implementation primitives. See xtask/README.md."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Task,
}

#[derive(Debug, Args, Clone, Copy, Default, PartialEq, Eq)]
#[group(multiple = false)]
pub struct SurfaceArgs {
    /// Select the app's Leptos Web client.
    #[arg(long)]
    pub web: bool,
    /// Select Theatre's production CMP mobile client (UNSUPPORTED); invalid for Studio.
    #[arg(long)]
    pub mobile: bool,
}

impl SurfaceArgs {
    pub fn surface(self) -> Option<Surface> {
        match (self.web, self.mobile) {
            (true, _) => Some(Surface::Web),
            (_, true) => Some(Surface::Mobile),
            _ => None,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Task {
    /// Build source-bound release artifacts locally or in CI; never deploys or signs.
    ReleaseBuild(crate::release_build::ReleaseBuildArgs),
    /// Resolve, package and explicitly promote source-bound releases (opt-in only).
    Release(crate::release::ReleaseArgs),
    /// Report a local operator-declared Hosting rollout record; no network or live verification.
    HostingReport(crate::hosting_report::HostingReportArgs),
    /// Read-only HTTPS comparison with one verified Web release; never publishes or logs in.
    HostingSmoke(crate::deploy::inspect::HostingSmokeArgs),
    /// Explicit static Web deployment of one app to Firebase Hosting (preview or live). A remote
    /// write; never invoked by build, run, verify, preflight or ci.
    Deploy(crate::deploy::DeployArgs),
    /// Explicit remote tester distribution (Firebase App Distribution, selected Theatre QA APK). A remote
    /// write; never invoked by build, run, verify, preflight or ci.
    Distribute(crate::distribute::DistributeArgs),
    /// Operator-only Google Cloud Secret Manager inventory, metadata status and explicit import (a
    /// remote write); never invoked by build, run, verify, preflight or ci.
    Secrets(crate::secrets::SecretsArgs),
    /// Retired with Kanidm; refuses before any effect.
    #[command(hide = true)]
    Kanidm(Retired),
    /// Native process inventory: list or validate it. Starts nothing.
    Services(crate::services::ServicesArgs),
    /// Capture a Compose @Preview PNG and semantics with Android Studio, without an emulator.
    ComposePreview(ComposePreviewArgs),
    /// Compare two local PNGs offline; differences are advisory, never a UX gate.
    VisualCompare(crate::visual_compare::VisualCompareArgs),
    /// Build a release Web bundle and inventory bytes; no browser or cache-hit claim.
    WebPerformance(crate::web_performance::WebPerformanceArgs),
    /// Measure one explicit Theatre APK's size groups and deltas offline; no upload or install.
    TheatreSize(crate::theatre_size::TheatreSizeArgs),
    /// Build a target with Cargo (Server + Identity, Forge) or Trunk (Web); dev profile unless --release.
    Build {
        #[arg(value_enum)]
        target: BuildTarget,
        #[command(flatten)]
        surface: SurfaceArgs,
        /// Optimized local artifact (cargo/trunk --release); not a production image.
        #[arg(long)]
        release: bool,
        /// Retired with the DBSP feature; refuses before any effect.
        #[arg(long, hide = true)]
        dbsp_experimental: bool,
    },
    /// Start the native Server + Identity (and Web for Studio/Theatre); supervise until Ctrl-C.
    Run {
        #[arg(value_enum)]
        target: RunTarget,
        #[command(flatten)]
        surface: SurfaceArgs,
        #[arg(long)]
        release: bool,
        /// Open the Web client in a browser once it is ready.
        #[arg(long)]
        open: bool,
        /// Retired with the disposable PostgreSQL cluster; refuses before any effect.
        #[arg(long, hide = true)]
        test_cluster: bool,
        /// Standalone Server Studio origin; Web profiles require their own Trunk origin.
        #[arg(long, value_name = "ORIGIN")]
        web_origin: Option<String>,
        /// Explicit local mail lane. Resend local records only a fixed synthetic message;
        /// Identity signup mail stays disabled. Firebase retains its separate proof adapter.
        #[arg(long, value_enum)]
        local_mail: Option<LocalMail>,
    },
    /// Explicit synthetic development data in the native development composition's own files.
    /// Never a gate, preflight selection or CI job.
    Seed {
        #[arg(value_enum)]
        target: SeedTarget,
        #[command(flatten)]
        options: SeedOptions,
    },
    /// Serve only Web; Studio requires an API, Theatre warns if anonymous catalog is unavailable.
    App {
        #[arg(value_enum)]
        target: AppTarget,
        #[command(flatten)]
        surface: SurfaceArgs,
        #[arg(long)]
        release: bool,
        /// Open the Web client in a browser once it is ready.
        #[arg(long)]
        open: bool,
    },
    /// Build, run or verify a package-local UI-system preview without an API or database.
    UiPreview {
        #[arg(value_enum)]
        renderer: Renderer,
        #[command(subcommand)]
        action: PreviewAction,
    },
    /// Build, verify or run the Theatre Desktop Preview: the one app over mock external ports on
    /// the JVM; no API, audio, native device or mobile preview host. `theatre-demo` is the
    /// historical name and runs the same Desktop Preview.
    #[command(name = "theatre-preview", visible_alias = "theatre-demo")]
    TheatrePreview {
        #[command(subcommand)]
        action: TheatrePreviewAction,
    },
    /// Build and inspect the production Theatre Android host's release APK/AAB; it composes
    /// `:shared` only. Nothing is signed, installed or distributed.
    TheatreHost {
        #[command(subcommand)]
        action: HostAction,
    },
    /// Run one verification suite.
    Verify {
        #[command(subcommand)]
        suite: VerifySuite,
    },
    /// Developer safety gate before pushing; --changed-only selects gates from the Git diff.
    Preflight {
        /// `handoff` (default) is the complete safety gate; `local` is a lighter DEV/CHECKPOINT
        /// subset (static repository checks, format, affected Rust and WASM lint) that requires
        /// --changed-only and never establishes a handoff.
        #[arg(long, value_enum, default_value_t = PreflightProfile::Handoff)]
        profile: PreflightProfile,
        /// Opt in to registered API Kani proofs, narrowed by changed paths when requested.
        #[arg(long)]
        kani: bool,
        /// Comparison base for --changed-only and the repository localization check.
        #[arg(long, default_value = DEFAULT_BASE, value_name = "REF")]
        base: String,
        /// Select gates affected by changes since the merge base with --base, including
        /// staged, unstaged and untracked files; uncertain impact runs every gate.
        #[arg(long)]
        changed_only: bool,
        /// Explicitly skip a gate; it is reported as NOT_RUN, never as PASS.
        #[arg(long = "skip", value_enum, value_name = "GATE")]
        skip: Vec<GateKind>,
        /// Print the selected gates and commands without running them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Run the CI contract: every job, or the one GitHub Actions selects with --job.
    Ci {
        #[arg(long, value_enum)]
        job: Option<CiJob>,
        /// Print the steps without running them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Report which tools, gates and surfaces are available on this machine.
    Doctor,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum VerifySuite {
    /// Isolate exactly one prebuilt native Rust test; no build, infrastructure or installation.
    NativeIsolation(crate::native_isolation::NativeIsolationArgs),
    /// Manual #336 Linux campaign: build first, then run the native auth layers under the guard.
    NativeAuthQualification(crate::native_qualification::QualificationArgs),
    /// Retired with provider authentication; refuses before any effect.
    #[command(hide = true)]
    TheatreAuth(Retired),
    /// Locked Quint model checks, optionally replayed against the registered pure core.
    Behavior {
        /// Select one stable executable capability from specs/behavior/catalog.json.
        #[arg(long, value_name = "ID")]
        capability: Option<String>,
        /// Explicitly install the locked development tool from the official npm registry.
        #[arg(long)]
        setup: bool,
        /// Replay complete Quint traces against independent production-core observations.
        #[arg(long)]
        conformance: bool,
    },
    /// Exact registered API bounded proofs; no normal build or CI requirement.
    Kani {
        /// Select a registered proposition (repeatable); omission selects all registered kernels.
        #[arg(long, value_enum)]
        harness: Vec<Kernel>,
    },
    /// Documents, links, JSON, skills, Python unit tests and whitespace.
    Repository,
    /// rustfmt, clippy -D warnings and tests for the whole Cargo workspace.
    Rust,
    /// Forge Rust lint/tests and offline Firebase Functions wrapper tests (Node 22+).
    Forge,
    /// Embedded Turso store foundation: lint, file-backed tests and the engine capability report.
    Turso,
    /// WASM clippy and Trunk builds for Studio and Theatre Web.
    Web,
    /// Script IR goldens, wire/media contract tests, WAV oracle and actual Opus intake inspection.
    Contracts,
    /// Bounded Script IR validator/encoder mutations on a disposable copy (offline Cargo).
    Mutation,
    /// Retired with the DBSP feature; refuses before any effect.
    #[command(hide = true)]
    Dbsp(Retired),
    /// Retired with PostgreSQL; refuses before any effect.
    #[command(hide = true)]
    Postgres(Retired),
}

impl Task {
    /// The retired command this invocation names, if any. Checked before anything else runs.
    pub fn retirement(&self) -> Option<Retirement> {
        match self {
            Task::Kanidm(_) => Some(Retirement::Kanidm),
            Task::Services(args) if args.is_retired() => Some(Retirement::ServicesLifecycle),
            Task::Deploy(args) if args.is_vps() => Some(Retirement::DeployVps),
            Task::ReleaseBuild(args) if args.is_legacy_api() => Some(Retirement::LegacyApiRelease),
            Task::Release(args) if args.names_legacy_api() => Some(Retirement::LegacyApiRelease),
            Task::Build {
                dbsp_experimental: true,
                ..
            } => Some(Retirement::BuildDbsp),
            Task::Run {
                test_cluster: true, ..
            } => Some(Retirement::RunTestCluster),
            Task::Seed { options, .. } if options.is_retired() => Some(Retirement::SeedEnvironment),
            Task::Verify { suite } => match suite {
                VerifySuite::Postgres(_) => Some(Retirement::VerifyPostgres),
                VerifySuite::Dbsp(_) => Some(Retirement::VerifyDbsp),
                VerifySuite::TheatreAuth(_) => Some(Retirement::VerifyTheatreAuth),
                _ => None,
            },
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(line: &str) -> Result<Task, clap::Error> {
        Cli::try_parse_from(std::iter::once("cargo xtask").chain(line.split_whitespace()))
            .map(|cli| cli.command)
    }

    #[test]
    fn synthetic_resend_and_explicit_firebase_are_distinct_local_lanes() {
        assert!(matches!(
            parse("run resend-local").unwrap(),
            Task::Run {
                target: RunTarget::ResendLocal,
                ..
            }
        ));
        for (name, expected) in [
            ("disabled", LocalMail::Disabled),
            ("firebase", LocalMail::Firebase),
            ("resend-local", LocalMail::ResendLocal),
        ] {
            assert!(
                matches!(parse(&format!("run cantos-theatre --web --local-mail {name}")).unwrap(),
                Task::Run { local_mail: Some(actual), .. } if actual == expected)
            );
        }
        assert!(parse("run cantos-theatre --web --local-mail live-resend").is_err());
    }

    #[test]
    fn the_command_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn native_isolation_requires_an_exact_target_and_bounded_deadline() {
        assert!(matches!(
            parse("verify native-isolation --test-binary /tmp/prebuilt --test fixture::exact --timeout-seconds 12 --dry-run").unwrap(),
            Task::Verify { suite: VerifySuite::NativeIsolation(args) }
            if args.dry_run && args.timeout_seconds == 12 && args.test == "fixture::exact"
        ));
        for line in [
            "verify native-isolation",
            "verify native-isolation --test-binary /tmp/prebuilt",
            "verify native-isolation --test-binary /tmp/prebuilt --test fixture --timeout-seconds 0",
            "verify native-isolation --test-binary /tmp/prebuilt --test fixture --timeout-seconds 121",
            "verify native-isolation --test-binary /tmp/prebuilt --test fixture --setup",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
    }

    #[test]
    fn seed_names_one_target_and_reset_is_explicit() {
        assert!(matches!(
            parse("seed theatre").unwrap(),
            Task::Seed {
                target: SeedTarget::Theatre,
                options: SeedOptions { reset: false, .. }
            }
        ));
        assert!(matches!(
            parse("seed theatre --reset").unwrap(),
            Task::Seed {
                options: SeedOptions { reset: true, .. },
                ..
            }
        ));
        for invalid in ["seed", "seed studio", "seed theatre --unknown"] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
        // The retired persistent lane still parses, hidden, so that it can refuse with its reason.
        for retired in [
            "seed theatre --database-url x",
            "seed theatre --studio-cookie-name arbitrary",
            "seed theatre --environment catalog-local --database-url-env DB --studio-token-env T",
        ] {
            let outcome = parse(retired);
            match outcome {
                Ok(task) => assert_eq!(
                    task.retirement(),
                    Some(Retirement::SeedEnvironment),
                    "{retired}"
                ),
                // An option that never existed is a plain usage error, not a retirement.
                Err(_) => assert!(retired.contains("--database-url x"), "{retired}"),
            }
        }
    }

    #[test]
    fn sample_seed_requires_explicit_sources_and_separates_serve_from_reset() {
        assert!(matches!(parse("seed theatre --sample-input /source/input.json --sample-media-source /source --sample-root /output --sample-serve").unwrap(), Task::Seed { options: SeedOptions { sample_serve: true, .. }, .. }));
        assert!(matches!(
            parse("seed theatre --sample-root /output --reset").unwrap(),
            Task::Seed {
                options: SeedOptions {
                    reset: true,
                    sample_input: None,
                    ..
                },
                ..
            }
        ));
        for invalid in [
            "seed theatre --sample-input /input.json",
            "seed theatre --sample-input /input.json --sample-root /output",
            "seed theatre --sample-media-source /source",
            "seed theatre --sample-serve",
            "seed theatre --sample-input /input.json --sample-media-source /source --sample-root /output --sample-serve --reset",
        ] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn every_retired_command_parses_with_any_arguments_and_names_its_retirement() {
        let cases = [
            ("kanidm plan --studio-admin x", Retirement::Kanidm),
            ("kanidm", Retirement::Kanidm),
            ("services up postgresql", Retirement::ServicesLifecycle),
            (
                "services --instance t down --all",
                Retirement::ServicesLifecycle,
            ),
            ("services logs kanidm", Retirement::ServicesLifecycle),
            ("services status", Retirement::ServicesLifecycle),
            ("verify postgres", Retirement::VerifyPostgres),
            (
                "verify postgres --suite schema --keep",
                Retirement::VerifyPostgres,
            ),
            ("verify dbsp", Retirement::VerifyDbsp),
            (
                "verify theatre-auth --suite firebase-emulator --dry-run",
                Retirement::VerifyTheatreAuth,
            ),
            (
                "deploy vps cantos-server --host root@h --status",
                Retirement::DeployVps,
            ),
            (
                "deploy vps cantos-server --host root@h --bootstrap --dry-run",
                Retirement::DeployVps,
            ),
            (
                "build cantos-server --dbsp-experimental",
                Retirement::BuildDbsp,
            ),
            (
                "run cantos-server --test-cluster",
                Retirement::RunTestCluster,
            ),
            (
                "run cantos-theatre --web --test-cluster",
                Retirement::RunTestCluster,
            ),
        ];
        for (line, expected) in cases {
            let task = parse(line).unwrap_or_else(|error| panic!("{line}: {error}"));
            assert_eq!(task.retirement(), Some(expected), "{line}");
        }
    }

    #[test]
    fn live_commands_are_not_retired_and_the_inventory_commands_remain() {
        for line in [
            "services list",
            "services validate",
            "verify turso",
            "verify rust",
            "build cantos-server",
            "run cantos-server",
            "seed theatre",
            "deploy firebase cantos-theatre --web --project p --channel live --dry-run",
            "doctor",
        ] {
            assert_eq!(parse(line).unwrap().retirement(), None, "{line}");
        }
    }

    #[test]
    fn retired_commands_are_hidden_from_help() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string();
        assert!(
            !help.contains("kanidm") && !help.contains("Kanidm"),
            "{help}"
        );
        for hidden in ["verify", "services", "deploy"] {
            let sub = command.find_subcommand_mut(hidden).unwrap();
            let text = sub.render_long_help().to_string();
            for retired in ["postgres", "dbsp", "theatre-auth", "vps", "logs", "down"] {
                assert!(!text.contains(retired), "{hidden}: {retired}\n{text}");
            }
        }
    }

    #[test]
    fn behavior_has_explicit_setup_capability_and_conformance_selection() {
        assert!(matches!(
            parse("verify behavior").unwrap(),
            Task::Verify {
                suite: VerifySuite::Behavior {
                    capability: None,
                    setup: false,
                    conformance: false
                }
            }
        ));
        assert!(
            matches!(parse("verify behavior --setup --capability cantos.media.private-audio-admission").unwrap(), Task::Verify {
            suite: VerifySuite::Behavior { capability: Some(id), setup: true, conformance: false }
        } if id == "cantos.media.private-audio-admission")
        );
        assert!(matches!(
            parse("verify behavior --conformance").unwrap(),
            Task::Verify {
                suite: VerifySuite::Behavior {
                    capability: None,
                    setup: false,
                    conformance: true
                }
            }
        ));
        for line in [
            "verify behavior --backend rust",
            "verify behavior --seed 7",
            "verify behavior --skip-witnesses",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
    }

    #[test]
    fn kani_accepts_only_registered_explicit_selectors() {
        assert!(
            matches!(parse("verify kani").unwrap(), Task::Verify { suite: VerifySuite::Kani { harness } } if harness.is_empty())
        );
        assert!(
            matches!(parse("verify kani --harness wav-payload --harness catalog-revision").unwrap(), Task::Verify { suite: VerifySuite::Kani { harness } } if harness == [Kernel::WavPayload, Kernel::CatalogRevision])
        );
        assert!(
            matches!(parse("verify kani --harness image-bounds").unwrap(), Task::Verify { suite: VerifySuite::Kani { harness } } if harness == [Kernel::ImageBounds])
        );
        assert!(
            matches!(parse("verify kani --harness byte-range --harness playback-grant --harness public-cover").unwrap(), Task::Verify { suite: VerifySuite::Kani { harness } } if harness == [Kernel::ByteRange, Kernel::PlaybackGrant, Kernel::PublicCover])
        );
        assert!(
            matches!(parse("verify kani --harness forge-attempt-ledger").unwrap(), Task::Verify { suite: VerifySuite::Kani { harness } } if harness == [Kernel::ForgeAttemptLedger])
        );
        for line in [
            "verify kani --harness theatre",
            "verify kani --harness wav",
            "verify kani --harness forge",
            "verify kani --package cantos-forge",
            "verify kani --no-default-checks",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
        assert!(
            matches!(parse("preflight --changed-only --kani --skip kani --dry-run").unwrap(), Task::Preflight { kani: true, changed_only: true, dry_run: true, skip, .. } if skip == [GateKind::Kani])
        );
        assert!(matches!(
            parse("preflight").unwrap(),
            Task::Preflight {
                kani: false,
                profile: PreflightProfile::Handoff,
                ..
            }
        ));
        assert!(matches!(
            parse("preflight --profile local --changed-only --base origin/refactor/cantos-native-turso --dry-run").unwrap(),
            Task::Preflight { profile: PreflightProfile::Local, changed_only: true, dry_run: true, ref base, .. }
                if base == "origin/refactor/cantos-native-turso"
        ));
        assert!(parse("preflight --profile quick").is_err());
    }

    #[test]
    fn parses_the_target_interface() {
        assert!(matches!(
            parse("build cantos-server").unwrap(),
            Task::Build {
                target: BuildTarget::CantosServer,
                release: false,
                ..
            }
        ));
        assert!(matches!(
            parse("build cantos-studio --web --release").unwrap(),
            Task::Build {
                target: BuildTarget::CantosStudio,
                surface: SurfaceArgs { web: true, .. },
                release: true,
                dbsp_experimental: false,
            }
        ));
        assert!(matches!(
            parse("build cantos-identity --release").unwrap(),
            Task::Build {
                target: BuildTarget::CantosIdentity,
                release: true,
                ..
            }
        ));
        assert!(matches!(
            parse("build cantos-forge --release").unwrap(),
            Task::Build {
                target: BuildTarget::CantosForge,
                release: true,
                ..
            }
        ));
        assert!(matches!(
            parse("build cantos-theatre --web").unwrap(),
            Task::Build {
                target: BuildTarget::CantosTheatre,
                ..
            }
        ));
        assert!(matches!(
            parse("build all").unwrap(),
            Task::Build {
                target: BuildTarget::All,
                ..
            }
        ));
        assert!(matches!(
            parse("run cantos-server").unwrap(),
            Task::Run {
                target: RunTarget::CantosServer,
                test_cluster: false,
                ..
            }
        ));
        assert!(matches!(
            parse("run cantos-studio --web --open").unwrap(),
            Task::Run {
                target: RunTarget::CantosStudio,
                open: true,
                ..
            }
        ));
        assert!(matches!(
            parse("run cantos-theatre --web").unwrap(),
            Task::Run {
                target: RunTarget::CantosTheatre,
                ..
            }
        ));
        assert!(matches!(
            parse("app cantos-studio --web").unwrap(),
            Task::App {
                target: AppTarget::CantosStudio,
                ..
            }
        ));
        for (line, suite) in [
            ("verify repository", VerifySuite::Repository),
            ("verify rust", VerifySuite::Rust),
            ("verify forge", VerifySuite::Forge),
            ("verify turso", VerifySuite::Turso),
            ("verify web", VerifySuite::Web),
            ("verify contracts", VerifySuite::Contracts),
            ("verify mutation", VerifySuite::Mutation),
        ] {
            assert!(
                matches!(parse(line).unwrap(), Task::Verify { suite: parsed } if parsed == suite)
            );
        }
        assert!(matches!(
            parse("preflight").unwrap(),
            Task::Preflight { changed_only: false, ref base, .. } if base == "origin/develop"
        ));
        assert!(matches!(
            parse("preflight --changed-only --base HEAD~1 --skip turso --skip web-build").unwrap(),
            Task::Preflight { changed_only: true, ref skip, .. }
                if skip == &[GateKind::Turso, GateKind::WebBuild]
        ));
        assert!(matches!(
            parse("ci").unwrap(),
            Task::Ci {
                job: None,
                dry_run: false
            }
        ));
        assert!(matches!(
            parse("ci --job rust --dry-run").unwrap(),
            Task::Ci {
                job: Some(CiJob::Rust),
                dry_run: true
            }
        ));
        // The retired jobs and gates are unknown values, not silently accepted.
        for line in [
            "ci --job postgres",
            "preflight --skip postgres",
            "preflight --skip dbsp",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
        assert!(matches!(parse("doctor").unwrap(), Task::Doctor));
    }

    #[test]
    fn parses_the_package_local_ui_preview_interface() {
        for (name, renderer) in [("leptos", Renderer::Leptos), ("cmp", Renderer::Cmp)] {
            for (verb, action) in [
                ("build", PreviewAction::Build),
                ("run", PreviewAction::Run { open: false }),
                ("verify", PreviewAction::Verify),
            ] {
                assert!(matches!(
                    parse(&format!("ui-preview {name} {verb}")).unwrap(),
                    Task::UiPreview { renderer: parsed_renderer, action: parsed_action }
                        if parsed_renderer == renderer && parsed_action == action
                ));
            }
        }
        assert!(matches!(
            parse("ui-preview leptos run --open").unwrap(),
            Task::UiPreview {
                renderer: Renderer::Leptos,
                action: PreviewAction::Run { open: true }
            }
        ));
        assert!(matches!(
            parse("ui-preview cmp android-build").unwrap(),
            Task::UiPreview {
                renderer: Renderer::Cmp,
                action: PreviewAction::AndroidBuild,
            }
        ));
        for line in [
            "ui-preview",
            "ui-preview leptos",
            "ui-preview mobile run",
            "ui-preview cmp deploy",
            "ui-preview leptos build --test-cluster",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
    }

    #[test]
    fn theatre_preview_is_desktop_only_and_its_historical_name_routes_to_it() {
        for (verb, action) in [
            ("build", TheatrePreviewAction::Build),
            ("verify", TheatrePreviewAction::Verify),
            ("run", TheatrePreviewAction::Run),
            ("android-build", TheatrePreviewAction::AndroidBuild),
            ("ios-build", TheatrePreviewAction::IosBuild),
        ] {
            for name in ["theatre-preview", "theatre-demo"] {
                assert!(
                    matches!(
                        parse(&format!("{name} {verb}")).unwrap(),
                        Task::TheatrePreview { action: parsed } if parsed == action
                    ),
                    "{name} {verb}"
                );
            }
        }
        for line in [
            "theatre-preview",
            "theatre-preview ios-build --kotlin-native-home /tmp/kotlin-native",
            "theatre-preview run --open",
            "theatre-preview deploy",
            "theatre-demo deploy",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
    }

    #[test]
    fn theatre_host_exposes_configured_release_and_explicit_debug_catalog_builds() {
        assert!(matches!(
            parse("theatre-host android-release").unwrap(),
            Task::TheatreHost {
                action: HostAction::AndroidRelease {
                    api_origin: None,
                    ..
                }
            }
        ));
        assert!(matches!(
            parse("theatre-host android-release --api-origin https://api.cantos.example").unwrap(),
            Task::TheatreHost {
                action: HostAction::AndroidRelease { api_origin: Some(origin), .. }
            } if origin == "https://api.cantos.example"
        ));
        assert!(matches!(
            parse("theatre-host android-debug --api-origin http://127.0.0.1:8080").unwrap(),
            Task::TheatreHost {
                action: HostAction::AndroidDebug { api_origin }
            } if api_origin == "http://127.0.0.1:8080"
        ));
        assert!(matches!(
            parse("theatre-host ios-build --kotlin-native-home /installed/native").unwrap(),
            Task::TheatreHost {
                action: HostAction::IosBuild { .. }
            }
        ));
        for line in [
            "theatre-host",
            "theatre-host android-build",
            "theatre-host android-release --install",
            "theatre-host android-debug",
            "theatre-host android-debug --api-origin http://127.0.0.1:8080 --install",
            // The retired provider issuer option is gone, not refused by a stub.
            "theatre-host android-release --native-issuer https://idm.example/oauth2/openid/cantos-theatre-native",
            "theatre-host android-debug --api-origin http://127.0.0.1:8080 --native-issuer https://idm.example/oauth2/openid/cantos-theatre-native",
            "theatre-host ios-release",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
    }

    #[test]
    fn distribute_firebase_separates_channel_variant_and_artifact() {
        use crate::distribute::{DistributeArgs, Provider};
        let Task::Distribute(DistributeArgs {
            provider: Provider::Firebase(args),
        }) = parse(
            "distribute firebase cantos-theatre --channel demo --groups cantos-internal --dry-run",
        )
        .unwrap()
        else {
            panic!("not a distribute task");
        };
        assert_eq!(format!("{:?}", args.channel), "Demo");
        assert_eq!(
            format!("{:?}", args.artifact),
            "Apk",
            "apk is the default artifact"
        );
        assert_eq!(args.groups, ["cantos-internal"]);
        assert!(args.testers.is_empty() && args.dry_run && args.app_id.is_none());

        // The optimized QA lane is its own explicit value; `demo` keeps its meaning.
        let Task::Distribute(DistributeArgs {
            provider: Provider::Firebase(args),
        }) = parse(
            "distribute firebase cantos-theatre --channel demo-qa --groups cantos-internal --dry-run",
        )
        .unwrap()
        else {
            panic!("not a distribute task");
        };
        assert_eq!(format!("{:?}", args.channel), "DemoQa");

        let Task::Distribute(DistributeArgs {
            provider: Provider::Firebase(args),
        }) = parse(
            "distribute firebase cantos-theatre --channel demo --artifact apk \
             --app-id 1:123:android:abc --groups a,b,a --testers q@example.com,r@example.com \
             --release-notes-file notes.txt",
        )
        .unwrap()
        else {
            panic!("not a distribute task");
        };
        assert_eq!(args.app_id.as_deref(), Some("1:123:android:abc"));
        // Raw splitting only: trimming and deduplication belong to the pure request.
        assert_eq!(args.groups, ["a", "b", "a"]);
        assert_eq!(args.testers.len(), 2);
        assert_eq!(
            args.release_notes_file.as_deref(),
            Some(std::path::Path::new("notes.txt"))
        );
        // Recognized-but-unavailable values still parse; the request refuses them as UNSUPPORTED.
        assert!(parse(
            "distribute firebase cantos-theatre --channel production --artifact aab --groups g"
        )
        .is_ok());
        assert!(parse(
            "distribute firebase cantos-theatre --channel demo --artifact aab --groups g"
        )
        .is_ok());
    }

    #[test]
    fn distribute_rejects_missing_unknown_or_scope_widening_arguments() {
        for line in [
            "distribute",
            "distribute firebase",
            "distribute firebase cantos-theatre --groups g",
            "distribute firebase cantos-theatre --channel nightly --groups g",
            "distribute firebase cantos-studio --channel demo --groups g",
            "distribute firebase cantos-server --channel demo --groups g",
            "distribute firebase cantos-theatre --channel demo --artifact ipa --groups g",
            "distribute firebase cantos-theatre --channel demo --groups g --release-notes a --release-notes-file b",
            "distribute firebase cantos-theatre --channel demo --groups g --no-build",
            "distribute firebase cantos-theatre --channel demo --groups g --apk-path x.apk",
            "distribute firebase cantos-theatre --channel demo --groups g --retry 3",
            "distribute firebase cantos-theatre --channel development --groups g --native-issuer https://idm.example/oauth2/openid/cantos-theatre-native",
            "distribute gcs cantos-theatre --channel demo --groups g",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
        // Empty entries survive parsing so the request can reject them as a usage error.
        let parsed = Cli::try_parse_from([
            "cargo xtask",
            "distribute",
            "firebase",
            "cantos-theatre",
            "--channel",
            "demo",
            "--groups",
            "a,,b",
        ])
        .unwrap();
        let Task::Distribute(args) = parsed.command else {
            panic!("not a distribute task");
        };
        let crate::distribute::Provider::Firebase(args) = args.provider;
        assert_eq!(args.groups, ["a", "", "b"]);
    }

    #[test]
    fn rejects_conflicting_or_unknown_arguments() {
        for line in [
            "build cantos-studio --web --mobile",
            "build cantos-web",
            "run all",
            "app cantos-server",
            "preflight --skip everything",
            "ci --job deploy",
            "verify",
            "deploy",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
    }

    #[test]
    fn surface_flags_map_to_surfaces() {
        assert_eq!(SurfaceArgs::default().surface(), None);
        let web = SurfaceArgs {
            web: true,
            mobile: false,
        };
        assert_eq!(web.surface(), Some(Surface::Web));
        let mobile = SurfaceArgs {
            web: false,
            mobile: true,
        };
        assert_eq!(mobile.surface(), Some(Surface::Mobile));
    }
}
