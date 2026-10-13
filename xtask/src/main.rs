//! `cargo xtask`: the single repository-level surface for building, running and verifying
//! Cantos. Repository-level operations go through cargo xtask; scripts and external tools are
//! implementation primitives, not competing orchestration interfaces.

mod build;
mod ci;
mod cli;
mod command;
mod compose_preview;
mod config;
mod doctor;
mod gate;
mod kani;
mod pipeline;
mod preflight;
mod probe;
mod process;
mod report;
mod run;
mod services;
mod theatre_demo;
mod tools;
mod ui_preview;
mod workspace;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

use build::{Profile, Refusal};
use cli::{Cli, Task};
use command::SystemRunner;
use report::{Report, Status};
use run::LaunchOptions;
use tools::SystemTools;
use workspace::Workspace;

/// Exit status for a malformed request, matching clap's own usage errors.
const USAGE: u8 = 2;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli.command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("xtask: error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(task: Task) -> Result<ExitCode> {
    let workspace = Workspace::discover()?;
    let tools = SystemTools::new(workspace.root());
    let mut runner = SystemRunner::new(workspace.root());
    match task {
        Task::Services(args) => services::run(&workspace, &args),
        Task::ComposePreview(args) => compose_preview::run(&workspace, &args, &tools),
        Task::Build {
            target,
            surface,
            release,
            dbsp_experimental,
        } => {
            let profile = Profile::from_release(release);
            let plan = build::plan(
                &workspace,
                target,
                surface.surface(),
                profile,
                dbsp_experimental,
            );
            let steps = match plan {
                Ok(steps) => steps,
                Err(refusal) => return Ok(refuse(refusal)),
            };
            let mut report = pipeline::execute("build", &steps, &tools, &mut runner);
            if target == build::BuildTarget::All {
                report.note("built, not run: cantos-forge. Not built: production mobile listening (theatre-demo builds only the isolated mock UI) and services/kanidm (configuration only)");
            }
            report.print();
            Ok(report.exit_code())
        }
        Task::Run {
            target,
            surface,
            release,
            open,
            test_cluster,
            web_origin,
        } => {
            let options = LaunchOptions {
                surface: surface.surface(),
                release,
                open,
                test_cluster,
                web_origin,
            };
            match run::plan_run(&workspace, target, &options) {
                Ok(plan) => run::launch(&workspace, &plan, &tools, &mut runner),
                Err(refusal) => Ok(refuse(refusal)),
            }
        }
        Task::App {
            target,
            surface,
            release,
            open,
        } => {
            let options = LaunchOptions {
                surface: surface.surface(),
                release,
                open,
                ..LaunchOptions::default()
            };
            match run::plan_app(&workspace, target, &options) {
                Ok(plan) => run::launch(&workspace, &plan, &tools, &mut runner),
                Err(refusal) => Ok(refuse(refusal)),
            }
        }
        Task::UiPreview { renderer, action } => {
            ui_preview::execute(&workspace, renderer, action, &tools, &mut runner)
        }
        Task::TheatreDemo { action } => {
            theatre_demo::execute(&workspace, action, &tools, &mut runner)
        }
        Task::Verify { suite } => {
            let mut steps = gate::steps(&gate::verify_suite(&suite), &workspace);
            if matches!(suite, cli::VerifySuite::Kani { .. }) {
                if let Err(error) = kani::prepare_configuration(&workspace, &mut steps) {
                    return Ok(kani::configuration_refusal("verify", &steps, error));
                }
            }
            let report = pipeline::execute("verify", &steps, &tools, &mut runner);
            report.print();
            Ok(report.exit_code())
        }
        Task::Preflight {
            kani,
            base,
            changed_only,
            skip,
            dry_run,
        } => preflight::run(
            &workspace,
            &preflight::PreflightOptions {
                kani,
                base,
                changed_only,
                skip: skip.into_iter().collect(),
                dry_run,
            },
            &tools,
            &mut runner,
        ),
        Task::Ci { job, dry_run } => Ok(ci::run(&workspace, job, dry_run, &tools, &mut runner)),
        Task::Doctor => Ok(doctor::run(&tools)),
    }
}

fn refuse(refusal: Refusal) -> ExitCode {
    match refusal {
        Refusal::Usage(message) => {
            eprintln!("xtask: error: {message}");
            ExitCode::from(USAGE)
        }
        Refusal::Unsupported { subject, reason } => {
            let mut report = Report::new(subject.clone());
            report.record(subject, Status::Unsupported, reason);
            report.print();
            report.exit_code()
        }
    }
}
