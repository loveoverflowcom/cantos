//! The command-line contract. Keep it small: every command maps to an implemented capability,
//! and anything without infrastructure is refused as UNSUPPORTED rather than simulated.

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    build::{BuildTarget, Surface},
    ci::CiJob,
    compose_preview::ComposePreviewArgs,
    config::DEFAULT_BASE,
    gate::GateKind,
    kani::Kernel,
    run::{AppTarget, RunTarget},
    seed::SeedTarget,
    theatre_demo::DemoAction,
    theatre_host::HostAction,
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
    /// Explicit static Web deployment of one app to Firebase Hosting (preview or live). A remote
    /// write; never invoked by build, run, verify, preflight or ci.
    Deploy(crate::deploy::DeployArgs),
    /// Explicit remote tester distribution (Firebase App Distribution, demo APK only). A remote
    /// write; never invoked by build, run, verify, preflight or ci.
    Distribute(crate::distribute::DistributeArgs),
    /// Studio Kanidm operator plan or read-only OIDC metadata check; no automatic bootstrap.
    Kanidm(crate::kanidm::KanidmArgs),
    /// Explicit Podman local infrastructure lifecycle; never used by build/test/run.
    Services(crate::services::ServicesArgs),
    /// Capture a Compose @Preview PNG and semantics with Android Studio, without an emulator.
    ComposePreview(ComposePreviewArgs),
    /// Compare two local PNGs offline; differences are advisory, never a UX gate.
    VisualCompare(crate::visual_compare::VisualCompareArgs),
    /// Build a release Web bundle and inventory bytes; no browser or cache-hit claim.
    WebPerformance(crate::web_performance::WebPerformanceArgs),
    /// Build a target with Cargo (API, Forge) or Trunk (Web); dev profile unless --release.
    Build {
        #[arg(value_enum)]
        target: BuildTarget,
        #[command(flatten)]
        surface: SurfaceArgs,
        /// Optimized local artifact (cargo/trunk --release); not a production image.
        #[arg(long)]
        release: bool,
        /// cantos-api only: compile the opt-in experimental DBSP incremental core.
        #[arg(long)]
        dbsp_experimental: bool,
    },
    /// Start API + Web for Studio/Theatre, or the API alone; supervise until Ctrl-C.
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
        /// Serve the API against the disposable cluster kept by `verify postgres --keep`.
        #[arg(long)]
        test_cluster: bool,
        /// API loopback origin; Web profiles require their Trunk origin (Studio 8081, Theatre 8082).
        #[arg(long, value_name = "ORIGIN")]
        web_origin: Option<String>,
    },
    /// Explicit synthetic development data, only in the cluster kept by `verify postgres --keep`.
    /// Never a gate, preflight selection or CI job.
    Seed {
        #[arg(value_enum)]
        target: SeedTarget,
        /// Drop only the recorded seed database and its record.
        #[arg(long)]
        reset: bool,
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
    /// Build, verify or run the isolated Theatre mock UI; no API or audio playback.
    TheatreDemo {
        #[command(subcommand)]
        action: DemoAction,
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
        /// Opt in to registered API Kani proofs, narrowed by changed paths when requested.
        #[arg(long)]
        kani: bool,
        /// Comparison base for --changed-only.
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
    /// WASM clippy and Trunk builds for Studio and Theatre Web.
    Web,
    /// Script IR goldens, wire/media contract tests, WAV oracle and actual Opus intake inspection.
    Contracts,
    /// Bounded Script IR validator/encoder mutations on a disposable copy (offline Cargo).
    Mutation,
    /// clippy and every cantos-api test with the experimental DBSP incremental core compiled in.
    Dbsp,
    /// PostgreSQL integration and restart tests in a new disposable cluster.
    Postgres {
        /// Retain the cluster for `run cantos-api --test-cluster`; the stop command is printed.
        #[arg(long)]
        keep: bool,
        /// Explicit narrow acceptance; omission runs the complete PostgreSQL gate.
        #[arg(long, value_enum, conflicts_with = "keep")]
        suite: Option<PostgresScope>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PostgresScope {
    /// Firebase enrollment SQL and existing SMTP registration migration compatibility only.
    TheatreFirebase,
    /// Legacy DBSP change-capture retirement and retained-data migration acceptance only.
    LegacyDbsp,
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
    fn the_command_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn seed_names_one_target_and_reset_is_explicit() {
        assert!(matches!(
            parse("seed theatre").unwrap(),
            Task::Seed {
                target: SeedTarget::Theatre,
                reset: false
            }
        ));
        assert!(matches!(
            parse("seed theatre --reset").unwrap(),
            Task::Seed { reset: true, .. }
        ));
        for invalid in ["seed", "seed studio", "seed theatre --database-url x"] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn postgres_keeps_full_default_and_requires_an_explicit_supported_narrow_suite() {
        assert!(matches!(
            parse("verify postgres").unwrap(),
            Task::Verify {
                suite: VerifySuite::Postgres {
                    keep: false,
                    suite: None,
                }
            }
        ));
        assert!(matches!(
            parse("verify postgres --suite theatre-firebase").unwrap(),
            Task::Verify {
                suite: VerifySuite::Postgres {
                    keep: false,
                    suite: Some(PostgresScope::TheatreFirebase),
                }
            }
        ));
        assert!(matches!(
            parse("verify postgres --suite legacy-dbsp").unwrap(),
            Task::Verify {
                suite: VerifySuite::Postgres {
                    keep: false,
                    suite: Some(PostgresScope::LegacyDbsp),
                }
            }
        ));
        for line in [
            "verify postgres --suite theatre-firebase --keep",
            "verify postgres --suite legacy-dbsp --keep",
            "verify postgres --suite audio",
            "verify postgres --suite all",
        ] {
            assert!(parse(line).is_err(), "{line}");
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
            Task::Preflight { kani: false, .. }
        ));
    }

    #[test]
    fn parses_the_target_interface() {
        assert!(matches!(
            parse("build cantos-api").unwrap(),
            Task::Build {
                target: BuildTarget::CantosApi,
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
            parse("build cantos-api --dbsp-experimental").unwrap(),
            Task::Build {
                target: BuildTarget::CantosApi,
                dbsp_experimental: true,
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
            parse("run cantos-api --test-cluster").unwrap(),
            Task::Run {
                target: RunTarget::CantosApi,
                test_cluster: true,
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
            ("verify web", VerifySuite::Web),
            ("verify contracts", VerifySuite::Contracts),
            ("verify mutation", VerifySuite::Mutation),
            ("verify dbsp", VerifySuite::Dbsp),
            (
                "verify postgres --keep",
                VerifySuite::Postgres {
                    keep: true,
                    suite: None,
                },
            ),
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
            parse("preflight --changed-only --base HEAD~1 --skip postgres --skip web-build").unwrap(),
            Task::Preflight { changed_only: true, ref skip, .. }
                if skip == &[GateKind::Postgres, GateKind::WebBuild]
        ));
        assert!(matches!(
            parse("ci").unwrap(),
            Task::Ci {
                job: None,
                dry_run: false
            }
        ));
        assert!(matches!(
            parse("ci --job postgres --dry-run").unwrap(),
            Task::Ci {
                job: Some(CiJob::Postgres),
                dry_run: true
            }
        ));
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
    fn theatre_demo_keeps_mock_and_native_actions_explicit() {
        for (verb, action) in [
            ("build", DemoAction::Build),
            ("verify", DemoAction::Verify),
            ("run", DemoAction::Run),
            ("android-build", DemoAction::AndroidBuild),
        ] {
            assert!(matches!(
                parse(&format!("theatre-demo {verb}")).unwrap(),
                Task::TheatreDemo { action: parsed } if parsed == action
            ));
        }
        assert!(matches!(
            parse("theatre-demo ios-build --kotlin-native-home /tmp/kotlin-native").unwrap(),
            Task::TheatreDemo { action: DemoAction::IosBuild { kotlin_native_home } }
                if kotlin_native_home == std::path::Path::new("/tmp/kotlin-native")
        ));
        for line in [
            "theatre-demo",
            "theatre-demo ios-build",
            "theatre-demo run --open",
            "theatre-demo deploy",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
    }

    #[test]
    fn theatre_host_has_only_the_release_inspection_and_an_explicit_debug_origin_build() {
        assert!(matches!(
            parse("theatre-host android-release").unwrap(),
            Task::TheatreHost {
                action: HostAction::AndroidRelease
            }
        ));
        assert!(matches!(
            parse("theatre-host android-debug --api-origin http://127.0.0.1:8080").unwrap(),
            Task::TheatreHost {
                action: HostAction::AndroidDebug { api_origin }
            } if api_origin == "http://127.0.0.1:8080"
        ));
        for line in [
            "theatre-host",
            "theatre-host android-build",
            "theatre-host android-release --install",
            "theatre-host android-debug",
            "theatre-host android-debug --api-origin http://127.0.0.1:8080 --install",
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
            "distribute firebase cantos-api --channel demo --groups g",
            "distribute firebase cantos-theatre --channel demo --artifact ipa --groups g",
            "distribute firebase cantos-theatre --channel demo --groups g --release-notes a --release-notes-file b",
            "distribute firebase cantos-theatre --channel demo --groups g --no-build",
            "distribute firebase cantos-theatre --channel demo --groups g --apk-path x.apk",
            "distribute firebase cantos-theatre --channel demo --groups g --retry 3",
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
            "app cantos-api",
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
