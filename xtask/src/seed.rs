//! Explicit synthetic backend data: the retained disposable cluster or a fully named existing
//! environment. The persistent lane only reconciles its immutable journal through Studio use
//! cases; reset/drop exists solely for the separately proven disposable cluster.

use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, ValueEnum};

use crate::{
    build::Refusal,
    command::{CommandSpec, Runner},
    config::{RETAINED_CLUSTER_RECORD, TEST_CLUSTER_RECORD},
    pipeline::{self, Step},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SeedTarget {
    /// Public Theatre catalog: novel, story and podcast Works published through Studio flows.
    Theatre,
}

/// Exactly the names selected by StudioOidc::session_name; no arbitrary cookie header input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StudioCookieName {
    #[value(name = "__Secure-cantos_studio")]
    Secure,
    #[value(name = "cantos_session")]
    Development,
}
impl StudioCookieName {
    fn as_str(self) -> &'static str {
        match self {
            Self::Secure => "__Secure-cantos_studio",
            Self::Development => "cantos_session",
        }
    }
}

/// Explicit persistent inputs name an existing environment and credentials by variable name.
/// Values never enter argv or a public receipt. No persistent reset lane exists.
#[derive(Args, Debug, Default, Clone, PartialEq, Eq)]
pub struct SeedOptions {
    #[arg(long)]
    pub reset: bool,
    #[arg(long, value_name = "NAME")]
    pub environment: Option<String>,
    #[arg(long, value_name = "ORIGIN")]
    pub api_origin: Option<String>,
    #[arg(long, value_name = "ORIGIN")]
    pub web_origin: Option<String>,
    #[arg(long, value_name = "VARIABLE")]
    pub database_url_env: Option<String>,
    #[arg(long, value_name = "VARIABLE")]
    pub studio_token_env: Option<String>,
    /// Defaults to the secure production spelling in named mode; never probes a second name.
    #[arg(long, value_enum, value_name = "NAME")]
    pub studio_cookie_name: Option<StudioCookieName>,
    #[arg(long, value_name = "ID")]
    pub rights_ref: Option<String>,
    #[arg(long, value_name = "PATH")]
    pub private_root: Option<std::path::PathBuf>,
    #[arg(long, value_name = "PATH")]
    pub publication_root: Option<std::path::PathBuf>,
    #[arg(long, value_name = "PATH")]
    pub record: Option<std::path::PathBuf>,
}

impl SeedOptions {
    fn arguments(&self) -> Result<Vec<String>, Refusal> {
        let persistent = self.environment.is_some();
        let fields = [
            ("--api-origin", self.api_origin.clone()),
            ("--web-origin", self.web_origin.clone()),
            ("--database-url-env", self.database_url_env.clone()),
            ("--studio-token-env", self.studio_token_env.clone()),
            ("--rights-ref", self.rights_ref.clone()),
            (
                "--private-root",
                self.private_root.as_ref().map(|p| p.display().to_string()),
            ),
            (
                "--publication-root",
                self.publication_root
                    .as_ref()
                    .map(|p| p.display().to_string()),
            ),
            (
                "--record",
                self.record.as_ref().map(|p| p.display().to_string()),
            ),
        ];
        if persistent && self.reset {
            return Err(Refusal::Usage(
                "persistent seed never supports --reset".into(),
            ));
        }
        let mut args = vec!["scripts/seed_theatre.py".into()];
        if persistent {
            args.extend([
                "--studio-cookie-name".into(),
                self.studio_cookie_name
                    .unwrap_or(StudioCookieName::Secure)
                    .as_str()
                    .into(),
            ]);
        } else if self.studio_cookie_name.is_some() {
            return Err(Refusal::Usage(
                "--studio-cookie-name requires --environment".into(),
            ));
        }
        if let Some(name) = &self.environment {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            {
                return Err(Refusal::Usage(
                    "--environment must be a lowercase name".into(),
                ));
            }
            args.extend(["--environment".into(), name.clone()]);
        }
        for (flag, value) in fields {
            match (persistent, value) {
                (true, Some(value)) if !value.is_empty() => args.extend([flag.into(), value]),
                (true, _) => {
                    return Err(Refusal::Usage(format!("persistent seed requires {flag}")))
                }
                (false, Some(_)) => {
                    return Err(Refusal::Usage(format!("{flag} requires --environment")))
                }
                (false, None) => {}
            }
        }
        if self.reset {
            args.push("--reset".into());
        }
        Ok(args)
    }
}

pub fn plan(
    workspace: &Workspace,
    target: SeedTarget,
    options: &SeedOptions,
) -> Result<Step, Refusal> {
    let SeedTarget::Theatre = target;
    let arguments = options.arguments()?;
    if options.environment.is_none()
        && !workspace.path(TEST_CLUSTER_RECORD).is_file()
        && !workspace.path(RETAINED_CLUSTER_RECORD).is_file()
    {
        return Err(Refusal::Usage(format!(
            "seed theatre needs the disposable cluster retained by \
             `cargo xtask verify postgres --keep` ({TEST_CLUSTER_RECORD} is missing); \
             it never uses DATABASE_URL"
        )));
    }
    let reset = options.reset;
    let command = CommandSpec::new("python3", workspace.root()).args(arguments);
    // Reset only drops the recorded database; seeding also builds and runs the media test.
    let tools: &[Tool] = if reset {
        &[Tool::Python3, Tool::Psql]
    } else if options.environment.is_some() {
        &[Tool::Python3, Tool::Cargo, Tool::Ffmpeg, Tool::Ffprobe]
    } else {
        &[
            Tool::Python3,
            Tool::Psql,
            Tool::Cargo,
            Tool::Ffmpeg,
            Tool::Ffprobe,
        ]
    };
    let name = if options.environment.is_some() {
        "seed theatre (reconcile owned synthetic dataset in a named environment)"
    } else if reset {
        "seed theatre --reset (drop the recorded seed database)"
    } else {
        "seed theatre (synthetic public catalog in the retained cluster)"
    };
    Ok(Step::new(name, tools, vec![command]))
}

pub fn execute(
    workspace: &Workspace,
    target: SeedTarget,
    options: &SeedOptions,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<std::result::Result<ExitCode, Refusal>> {
    let step = match plan(workspace, target, options) {
        Ok(step) => step,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut report = pipeline::execute("seed", &[step], tools, runner);
    if !options.reset && report.succeeded() {
        report.note("Seed receipt lists Work, Unit, Publication, Rendition and storage provenance; native playback acceptance is separate");
    }
    report.print();
    Ok(Ok(report.exit_code()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::testing::RecordingRunner, report::Status, tools::testing::FakeTools};
    use std::fs;

    /// A throwaway workspace root; removed when the guard drops.
    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn workspace_with_record(present: bool) -> (Root, Workspace) {
        let root = std::env::temp_dir().join(format!(
            "cantos-xtask-seed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        if present {
            let record = root.join(TEST_CLUSTER_RECORD);
            fs::create_dir_all(record.parent().unwrap()).unwrap();
            fs::write(record, "{}").unwrap();
        }
        let workspace = Workspace::fixture(&root).unwrap();
        (Root(root), workspace)
    }

    #[test]
    fn seed_and_reset_are_one_explicit_primitive_step_each() {
        let (_dir, workspace) = workspace_with_record(true);
        let seed = plan(&workspace, SeedTarget::Theatre, &SeedOptions::default()).unwrap();
        assert_eq!(
            seed.commands[0].display(workspace.root()),
            "python3 scripts/seed_theatre.py"
        );
        assert!(seed.tools.contains(&Tool::Ffmpeg) && seed.tools.contains(&Tool::Cargo));
        let reset = plan(
            &workspace,
            SeedTarget::Theatre,
            &SeedOptions {
                reset: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            reset.commands[0].display(workspace.root()),
            "python3 scripts/seed_theatre.py --reset"
        );
        assert_eq!(reset.tools, [Tool::Python3, Tool::Psql]);
    }

    #[test]
    fn no_retained_cluster_is_a_usage_refusal_before_any_command() {
        let (_dir, workspace) = workspace_with_record(false);
        for reset in [false, true] {
            let mut runner = RecordingRunner::default();
            let outcome = execute(
                &workspace,
                SeedTarget::Theatre,
                &SeedOptions {
                    reset,
                    ..Default::default()
                },
                &FakeTools::all(),
                &mut runner,
            )
            .unwrap();
            let Err(Refusal::Usage(message)) = outcome else {
                panic!("expected a usage refusal");
            };
            assert!(message.contains("verify postgres --keep"));
            assert!(message.contains("never uses DATABASE_URL"));
            assert!(runner.commands.is_empty());
        }
    }

    #[test]
    fn persistent_seed_requires_every_input_and_refuses_reset_before_effects() {
        let (_dir, workspace) = workspace_with_record(false);
        let options = SeedOptions {
            environment: Some("catalog-local".into()),
            api_origin: Some("https://api.example.org".into()),
            web_origin: Some("https://studio.example.org".into()),
            database_url_env: Some("SEED_DB".into()),
            studio_token_env: Some("SEED_TOKEN".into()),
            studio_cookie_name: None,
            rights_ref: Some("synthetic-owned".into()),
            private_root: Some("/private".into()),
            publication_root: Some("/publication".into()),
            record: Some("/receipt.json".into()),
            reset: false,
        };
        let step = plan(&workspace, SeedTarget::Theatre, &options).unwrap();
        let command = step.commands[0].display(workspace.root());
        assert!(command.contains("--environment catalog-local"));
        assert!(command.contains("--database-url-env SEED_DB"));
        assert!(command.contains("--studio-cookie-name __Secure-cantos_studio"));
        let development = plan(
            &workspace,
            SeedTarget::Theatre,
            &SeedOptions {
                studio_cookie_name: Some(StudioCookieName::Development),
                ..options.clone()
            },
        )
        .unwrap();
        assert!(development.commands[0]
            .display(workspace.root())
            .contains("--studio-cookie-name cantos_session"));
        assert!(!command.contains("--reset"));
        assert!(matches!(
            plan(
                &workspace,
                SeedTarget::Theatre,
                &SeedOptions {
                    reset: true,
                    ..options.clone()
                }
            ),
            Err(Refusal::Usage(_))
        ));
        assert!(matches!(
            plan(
                &workspace,
                SeedTarget::Theatre,
                &SeedOptions {
                    record: None,
                    ..options
                }
            ),
            Err(Refusal::Usage(_))
        ));
    }

    #[test]
    fn independent_retained_ownership_is_enough_to_plan_seed_without_full_fixture_claims() {
        let (_dir, workspace) = workspace_with_record(false);
        let record = workspace.path(RETAINED_CLUSTER_RECORD);
        fs::create_dir_all(record.parent().unwrap()).unwrap();
        fs::write(record, "{}").unwrap();
        let step = plan(&workspace, SeedTarget::Theatre, &SeedOptions::default()).unwrap();
        assert_eq!(
            step.commands[0].display(workspace.root()),
            "python3 scripts/seed_theatre.py"
        );
        assert!(!workspace.path(TEST_CLUSTER_RECORD).exists());
    }

    #[test]
    fn a_missing_postgres_client_is_unsupported_and_runs_nothing() {
        let (_dir, workspace) = workspace_with_record(true);
        let step = plan(&workspace, SeedTarget::Theatre, &SeedOptions::default()).unwrap();
        let mut runner = RecordingRunner::default();
        let report = pipeline::execute(
            "seed",
            &[step],
            &FakeTools::without(&[Tool::Psql]),
            &mut runner,
        );
        assert!(runner.commands.is_empty());
        assert_eq!(report.outcomes[0].status, Status::Unsupported);
    }
}
