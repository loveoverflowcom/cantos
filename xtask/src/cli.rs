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
    run::{AppTarget, RunTarget},
    theatre_demo::DemoAction,
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
    /// Studio Kanidm operator plan or read-only OIDC metadata check; no automatic bootstrap.
    Kanidm(crate::kanidm::KanidmArgs),
    /// Explicit Podman local infrastructure lifecycle; never used by build/test/run.
    Services(crate::services::ServicesArgs),
    /// Capture a Compose @Preview PNG and semantics with Android Studio, without an emulator.
    ComposePreview(ComposePreviewArgs),
    /// Compare two local PNGs offline; differences are advisory, never a UX gate.
    VisualCompare(crate::visual_compare::VisualCompareArgs),
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
    /// Build what the target needs, start its local processes and supervise them until Ctrl-C.
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
        /// CANTOS_WEB_ORIGIN for the API (a loopback http origin, e.g. http://127.0.0.1:8081).
        #[arg(long, value_name = "ORIGIN")]
        web_origin: Option<String>,
    },
    /// Serve an app's Web client against an already running backend; never builds or starts it.
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
    },
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
        for line in [
            "verify kani --harness theatre",
            "verify kani --harness wav",
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
            ("verify web", VerifySuite::Web),
            ("verify contracts", VerifySuite::Contracts),
            ("verify mutation", VerifySuite::Mutation),
            ("verify dbsp", VerifySuite::Dbsp),
            (
                "verify postgres --keep",
                VerifySuite::Postgres { keep: true },
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
