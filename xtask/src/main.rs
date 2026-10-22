//! `cargo xtask`: the single repository-level surface for building, running and verifying
//! Cantos. Repository-level operations go through cargo xtask; scripts and external tools are
//! implementation primitives, not competing orchestration interfaces.

mod behavior;
mod build;
mod capture;
mod ci;
mod cli;
mod command;
mod compose_preview;
mod config;
mod deploy;
mod distribute;
mod doctor;
mod firebase_cli;
mod gate;
mod hosting_report;
mod kani;
mod mobile_toolchain;
mod native_dev;
mod native_isolation;
mod native_qualification;
mod pipeline;
mod preflight;
mod probe;
mod process;
mod release;
mod release_build;
mod report;
mod retired;
mod run;
mod run_files;
mod seed;
mod services;
mod theatre_host;
mod theatre_preview;
mod theatre_size;
mod tools;
mod ui_preview;
mod visual_compare;
mod web_performance;
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
    // A retired command refuses before anything else: no workspace read, tool probe or process.
    if let Some(retirement) = task.retirement() {
        return Ok(refuse(retirement.refusal()));
    }
    let workspace = Workspace::discover()?;
    let tools = SystemTools::new(workspace.root());
    let mut runner = SystemRunner::new(workspace.root());
    match task {
        Task::ReleaseBuild(args) => release_build::run(&workspace, &args, &tools, &mut runner),
        Task::Release(args) => release::run(&workspace, &args, &tools, &mut runner),
        Task::HostingReport(args) => hosting_report::run(&workspace, &args, &tools, &mut runner),
        Task::HostingSmoke(args) => deploy::inspect::run(&workspace, &args, &tools),
        Task::Deploy(args) => deploy::execute(&workspace, &args, &tools, &mut runner),
        Task::Distribute(args) => distribute::execute(&workspace, &args, &tools, &mut runner),
        Task::Kanidm(_) => unreachable!("retired commands are refused before dispatch"),
        Task::Services(args) => services::run(&workspace, &args),
        Task::ComposePreview(args) => compose_preview::run(&workspace, &args, &tools),
        Task::VisualCompare(args) => visual_compare::run(&workspace, &args),
        Task::WebPerformance(args) => web_performance::run(&workspace, &args, &tools, &mut runner),
        Task::TheatreSize(args) => theatre_size::run(&workspace, &args, &tools, &mut runner),
        Task::Build {
            target,
            surface,
            release,
            dbsp_experimental: _,
        } => {
            let profile = Profile::from_release(release);
            let plan = build::plan(&workspace, target, surface.surface(), profile);
            let steps = match plan {
                Ok(steps) => steps,
                Err(refusal) => return Ok(refuse(refusal)),
            };
            let mut report = pipeline::execute("build", &steps, &tools, &mut runner);
            if target == build::BuildTarget::All {
                report.note("built, not run: cantos-server, cantos-identity and cantos-forge. Not built: production mobile listening (theatre-preview builds only the JVM Desktop Preview over mock ports)");
            }
            report.print();
            Ok(report.exit_code())
        }
        Task::Run {
            target,
            surface,
            release,
            open,
            test_cluster: _,
            web_origin,
        } => {
            let options = LaunchOptions {
                surface: surface.surface(),
                release,
                open,
                web_origin,
            };
            match run::plan_run(&workspace, target, &options) {
                Ok(plan) => run::launch(&workspace, &plan, &tools, &mut runner),
                Err(refusal) => Ok(refuse(refusal)),
            }
        }
        Task::Seed { target, options } => {
            match seed::execute(&workspace, target, &options, &tools, &mut runner)? {
                Ok(code) => Ok(code),
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
        Task::TheatrePreview { action } => {
            theatre_preview::execute(&workspace, action, &tools, &mut runner)
        }
        Task::TheatreHost { action } => {
            theatre_host::execute(&workspace, action, &tools, &mut runner)
        }
        Task::Verify { suite } => {
            if let cli::VerifySuite::NativeIsolation(args) = &suite {
                return native_isolation::execute(&workspace, args);
            }
            if let cli::VerifySuite::NativeAuthQualification(args) = &suite {
                return native_qualification::execute(&workspace, args, &tools, &mut runner);
            }
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
            profile,
            kani,
            base,
            changed_only,
            skip,
            dry_run,
        } => {
            let options = preflight::PreflightOptions {
                profile,
                kani,
                base,
                changed_only,
                skip: skip.into_iter().collect(),
                dry_run,
            };
            if let Err(message) = options.validate() {
                return Ok(refuse(Refusal::Usage(message)));
            }
            preflight::run(&workspace, &options, &tools, &mut runner)
        }
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
