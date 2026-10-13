//! Explicit local infrastructure lifecycle. No build/test/run path calls `up`.
mod config;
mod lifecycle;
mod manifest;
mod podman;

#[cfg(all(test, unix))]
mod tests;

use std::{collections::BTreeMap, path::Path, process::ExitCode, time::Duration};

use anyhow::{ensure, Result};
use clap::{Args, Subcommand};

use crate::{
    report::{Report, Status},
    workspace::Workspace,
};
use manifest::{Manifest, Runtime};
use podman::{Podman, SystemPodman};

#[derive(Debug, Args)]
pub struct ServicesArgs {
    /// Existing local resource namespace; shared across checkouts unless changed explicitly.
    #[arg(long, default_value = "local")]
    pub instance: String,
    #[command(subcommand)]
    pub action: Action,
}

#[derive(Debug, Subcommand)]
pub enum Action {
    /// List the typed inventory without probing or starting infrastructure.
    List,
    /// Validate the inventory and public examples without Podman, secrets or containers.
    Validate,
    /// Explicitly start local container infrastructure; --all excludes optional Kanidm.
    Up {
        #[command(flatten)]
        target: Target,
        /// Readiness deadline after container creation/start. Every Podman command is bounded too.
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u16).range(1..=300))]
        timeout_seconds: u16,
    },
    /// Observe container state; native/external services are explicitly unobserved.
    Status,
    /// Print at most the last200 log lines from the inspected owned container ID.
    Logs { service: String },
    /// Stop owned containers only. Never removes a container, volume or data.
    Down {
        #[command(flatten)]
        target: Target,
    },
}

#[derive(Debug, Args)]
pub struct Target {
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub service: Option<String>,
    #[arg(long)]
    pub all: bool,
}

impl Target {
    fn names(&self, up: bool) -> Vec<&str> {
        if self.all {
            if up {
                vec!["postgresql"]
            } else {
                vec!["kanidm", "postgresql"]
            }
        } else {
            self.service.as_deref().into_iter().collect()
        }
    }
}

pub fn validate(root: &Path) -> Result<Manifest> {
    let manifest = Manifest::load(root)?;
    config::validate_kanidm(
        &manifest::read_small(&root.join("services/kanidm/server.toml.example"))?,
        Some(
            manifest
                .service("kanidm")?
                .container
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Kanidm container missing"))?
                .default_port,
        ),
    )?;
    for (service, expected) in [
        (
            "postgresql",
            vec!["CANTOS_POSTGRES_PORT", "CANTOS_POSTGRES_PASSWORD_FILE"],
        ),
        (
            "kanidm",
            vec!["CANTOS_KANIDM_PORT", "CANTOS_KANIDM_CONFIG_DIR"],
        ),
    ] {
        let text = manifest::read_small(&root.join(format!("services/{service}/env.example")))?;
        let mut keys = std::collections::BTreeSet::new();
        for line in text
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("invalid service env.example assignment"))?;
            ensure!(
                expected.contains(&key) && keys.insert(key),
                "unknown or duplicate env.example key"
            );
            ensure!(!value.is_empty(), "empty env.example value");
            if key.ends_with("_PORT") {
                ensure!(value.parse::<u16>()? >= 1024, "invalid env.example port");
            } else {
                ensure!(
                    value.starts_with("/absolute/private/"),
                    "examples must use placeholder private paths"
                );
            }
        }
        ensure!(
            keys.len() == expected.len(),
            "missing service env.example keys"
        );
    }
    Ok(manifest)
}

pub fn run(workspace: &Workspace, args: &ServicesArgs) -> Result<ExitCode> {
    let manifest = validate(workspace.root())?;
    ensure!(
        config::instance_valid(&args.instance),
        "invalid instance name; use 1–32 lowercase letters/digits/hyphens"
    );
    match &args.action {
        Action::Validate => {
            let mut report = Report::new("services validate");
            report.record(
                "manifest and examples",
                Status::Pass,
                "typed/static validation only; no Podman/TLS/runtime evidence",
            );
            report.print();
            return Ok(report.exit_code());
        }
        Action::List => {
            for (name, service) in &manifest.services {
                println!(
                    "{name}: {:?} / {:?}; owner={}",
                    service.runtime, service.implementation, service.owner
                );
                println!(
                    "  required={:?}; optional={:?}; ports={:?}",
                    service.required_dependencies, service.optional_dependencies, service.ports
                );
                println!(
                    "  development={}; production={}",
                    service.development, service.production
                );
                println!(
                    "  health={}; persistence={}; lifecycle={}",
                    service.health, service.persistence, service.lifecycle
                );
            }
            return Ok(ExitCode::SUCCESS);
        }
        Action::Up { target, .. } | Action::Down { target } => {
            for name in target.names(matches!(args.action, Action::Up { .. })) {
                ensure_container(&manifest, name)?;
            }
        }
        Action::Logs { service } => ensure_container(&manifest, service)?,
        Action::Status => {}
    }
    let mut report = Report::new("services");
    let mut podman = SystemPodman::new()?;
    if let Err(error) = lifecycle::available(&mut podman) {
        let status = if error.to_string().starts_with("UNSUPPORTED:") {
            Status::Unsupported
        } else {
            Status::Fail
        };
        report.record("runtime", status, format!("{error:#}"));
        report.print();
        return Ok(report.exit_code());
    }
    let env: BTreeMap<_, _> = [
        "CANTOS_POSTGRES_PORT",
        "CANTOS_POSTGRES_PASSWORD_FILE",
        "CANTOS_KANIDM_PORT",
        "CANTOS_KANIDM_CONFIG_DIR",
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok().map(|value| (key.to_owned(), value)))
    .collect();
    execute(
        workspace.root(),
        &manifest,
        args,
        &env,
        &mut podman,
        &mut report,
    )?;
    if matches!(args.action, Action::Status) {
        print!("{}", report.render_table());
        Ok(
            if report
                .outcomes
                .iter()
                .any(|outcome| matches!(outcome.status, Status::Fail | Status::Unsupported))
            {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            },
        )
    } else {
        report.print();
        Ok(report.exit_code())
    }
}

fn ensure_container(manifest: &Manifest, name: &str) -> Result<()> {
    let service = manifest.service(name)?;
    ensure!(service.runtime == Runtime::Podman, "UNSUPPORTED: {name} is {:?}; native lifecycle uses cargo xtask run and external services are operator-managed", service.runtime);
    Ok(())
}

fn execute(
    root: &Path,
    manifest: &Manifest,
    args: &ServicesArgs,
    env: &BTreeMap<String, String>,
    podman: &mut impl Podman,
    report: &mut Report,
) -> Result<()> {
    match &args.action {
        Action::Up {
            target,
            timeout_seconds,
        } => {
            // Resolve all prerequisites before the first effect; never partially start --all
            // because the next service's private inputs were missing.
            let plans = target
                .names(true)
                .iter()
                .map(|name| {
                    let container = manifest
                        .service(name)?
                        .container
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("container configuration missing"))?;
                    config::Plan::resolve(root, &args.instance, name, container, env)
                })
                .collect::<Result<Vec<_>>>()?;
            for plan in plans {
                let result = lifecycle::up(
                    podman,
                    &plan,
                    Duration::from_secs(u64::from(*timeout_seconds)),
                );
                let failed = result.is_err();
                record(report, &plan.service, result);
                if failed {
                    break;
                }
            }
        }
        Action::Down { target } => {
            for name in target.names(false) {
                let result = lifecycle::down(podman, &args.instance, name);
                let failed = result.is_err();
                record(report, name, result);
                if failed {
                    break;
                }
            }
        }
        Action::Logs { service } => {
            record(
                report,
                service,
                lifecycle::logs(podman, &args.instance, service).map(|logs| {
                    print!("{logs}");
                    "bounded log read from owned ID".to_owned()
                }),
            );
        }
        Action::Status => {
            for (name, service) in &manifest.services {
                if service.runtime == Runtime::Podman {
                    record(
                        report,
                        name,
                        lifecycle::inspect(podman, &args.instance, name).map(|state| {
                            state
                                .map(|state| state.description())
                                .unwrap_or_else(|| "observed ABSENT".into())
                        }),
                    );
                } else {
                    report.record(
                        name,
                        Status::NotRun,
                        format!(
                            "NOT_OBSERVED {:?}; no implemented runtime probe",
                            service.runtime
                        ),
                    );
                }
            }
        }
        Action::List | Action::Validate => {}
    }
    Ok(())
}

fn record(report: &mut Report, name: &str, result: Result<String>) {
    match result {
        Ok(detail) => report.record(name, Status::Pass, detail),
        Err(error) => report.record(
            name,
            if error.to_string().starts_with("UNSUPPORTED:") {
                Status::Unsupported
            } else {
                Status::Fail
            },
            format!("{error:#}"),
        ),
    }
}
