//! The command-line contract. Keep it small: every command maps to an implemented capability,
//! and anything without infrastructure is refused as UNSUPPORTED rather than simulated.

use clap::{Args, Parser, Subcommand};

use crate::{
    build::{BuildTarget, Surface},
    ci::CiJob,
    config::DEFAULT_BASE,
    gate::GateKind,
    run::{AppTarget, RunTarget},
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
    /// Select the app's CMP mobile client (UNSUPPORTED: no Gradle project yet).
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
    /// Build a target with Cargo (API) or Trunk (Web); dev profile unless --release.
    Build {
        #[arg(value_enum)]
        target: BuildTarget,
        #[command(flatten)]
        surface: SurfaceArgs,
        /// Optimized local artifact (cargo/trunk --release); not a production image.
        #[arg(long)]
        release: bool,
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
    /// Run one verification suite.
    Verify {
        #[command(subcommand)]
        suite: VerifySuite,
    },
    /// Developer safety gate before pushing; --changed-only selects gates from the Git diff.
    Preflight {
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
    /// Documents, links, JSON, skills, Python unit tests and whitespace.
    Repository,
    /// rustfmt, clippy -D warnings and tests for the whole Cargo workspace.
    Rust,
    /// WASM clippy and Trunk builds for Studio and Theatre Web.
    Web,
    /// Script IR goldens, wire/Script IR/WAV/TTS receipt contract tests and the WAV oracle.
    Contracts,
    /// Bounded Script IR validator/encoder mutations on a disposable copy (offline Cargo).
    Mutation,
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
                release: true
            }
        ));
        assert!(matches!(
            parse("build cantos --web").unwrap(),
            Task::Build {
                target: BuildTarget::Cantos,
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
            parse("run cantos --web").unwrap(),
            Task::Run {
                target: RunTarget::Cantos,
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
