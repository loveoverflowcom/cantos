//! `run` and `app`: local process plans and their supervised execution.
//!
//! `run` builds what the target needs and starts every implemented local process for it.
//! `app` starts only an app's Web frontend and requires its backend to be running already.
//! Neither command provisions PostgreSQL, Kanidm, object storage or any provider: the API uses
//! an explicit `DATABASE_URL` or the disposable cluster retained by `verify postgres --keep`.

use std::{net::SocketAddr, path::PathBuf, process::ExitCode, time::Duration};

use anyhow::{bail, Result};
use clap::ValueEnum;

use crate::{
    build::{self, app_surface, Profile, Refusal, Surface},
    command::{CommandSpec, Runner},
    config::{loopback, App, API_PORT, API_SERVER_BIN, TEST_CLUSTER_RECORD},
    pipeline::{self, Step},
    probe::{self, Readiness},
    process::{Supervisor, Wait},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

const API_READY_TIMEOUT: Duration = Duration::from_secs(60);
/// Trunk compiles the WASM bundle before it serves the first page.
const TRUNK_READY_TIMEOUT: Duration = Duration::from_secs(900);

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RunTarget {
    /// The Axum API host (`cantos-server`) in the foreground.
    CantosApi,
    /// Cantos Studio: with --web, the API plus a `trunk serve` frontend.
    CantosStudio,
    /// Cantos Theatre: with --web, the `trunk serve` skeleton (no backend exists).
    Cantos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AppTarget {
    /// Studio frontend against an API already listening on 127.0.0.1:8080.
    CantosStudio,
    /// Theatre frontend; it has no backend.
    Cantos,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    pub surface: Option<Surface>,
    pub release: bool,
    pub open: bool,
    pub test_cluster: bool,
    pub web_origin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requirement {
    /// An environment variable must be set; its value is never printed.
    EnvVar {
        name: &'static str,
        hint: String,
    },
    File {
        path: PathBuf,
        hint: String,
    },
    /// Another process must already accept connections here.
    Listening {
        name: &'static str,
        addr: SocketAddr,
        hint: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub command: CommandSpec,
    pub tools: Vec<Tool>,
    pub addr: SocketAddr,
    pub readiness: Readiness,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub requirements: Vec<Requirement>,
    pub prepare: Vec<Step>,
    pub services: Vec<Service>,
    pub open: Option<String>,
}

fn api_addr() -> SocketAddr {
    loopback(API_PORT)
}

/// `CANTOS_WEB_ORIGIN` must stay a loopback HTTP origin, matching the development host.
pub fn validate_web_origin(origin: &str) -> Result<(), Refusal> {
    let host_port = origin
        .strip_prefix("http://")
        .filter(|rest| !rest.contains('/'))
        .and_then(|rest| rest.rsplit_once(':'));
    match host_port {
        Some((host, port))
            if matches!(host, "127.0.0.1" | "localhost") && port.parse::<u16>().is_ok() =>
        {
            Ok(())
        }
        _ => Err(Refusal::Usage(format!(
            "--web-origin must look like http://127.0.0.1:<port>, got `{origin}`"
        ))),
    }
}

fn api_service(
    workspace: &Workspace,
    profile: Profile,
    test_cluster: bool,
    web_origin: Option<&str>,
) -> (Requirement, Service) {
    let (requirement, command) = if test_cluster {
        // The primitive validates that the record names the disposable test database.
        (
            Requirement::File {
                path: workspace.path(TEST_CLUSTER_RECORD),
                hint: "create and retain one with `cargo xtask verify postgres --keep`".into(),
            },
            CommandSpec::new("python3", workspace.root()).args(["scripts/serve_studio.py"]),
        )
    } else {
        let binary = workspace
            .path("target")
            .join(profile.cargo_dir())
            .join(API_SERVER_BIN);
        (
            Requirement::EnvVar {
                name: "DATABASE_URL",
                hint: "export a migrated local app-role URL (services/cantos-api/README.md) \
                       or pass --test-cluster"
                    .into(),
            },
            CommandSpec::new(binary.display().to_string(), workspace.root())
                .env("CANTOS_ENV", "development")
                .env("CANTOS_BIND_ADDRESS", api_addr().to_string()),
        )
    };
    let command = match web_origin {
        Some(origin) => command.env("CANTOS_WEB_ORIGIN", origin),
        None => command,
    };
    let service = Service {
        name: API_SERVER_BIN.into(),
        command,
        tools: if test_cluster {
            vec![Tool::Python3]
        } else {
            Vec::new()
        },
        addr: api_addr(),
        readiness: Readiness::Tcp,
        timeout: API_READY_TIMEOUT,
    };
    (requirement, service)
}

fn trunk_service(workspace: &Workspace, app: App, profile: Profile) -> Service {
    Service {
        name: format!("{} web (trunk serve)", app.cli_name()),
        command: build::trunk(workspace, app, "serve", profile),
        tools: vec![Tool::Cargo, Tool::Wasm32Target, Tool::Trunk],
        addr: loopback(app.web_port()),
        readiness: Readiness::Http {
            path: "/",
            marker: app.web_marker(),
        },
        timeout: TRUNK_READY_TIMEOUT,
    }
}

fn api_build(workspace: &Workspace, profile: Profile) -> Step {
    Step::new(
        format!("build {API_SERVER_BIN} ({})", profile.label()),
        &[Tool::Cargo],
        vec![build::api_binaries(
            workspace,
            profile,
            Some(API_SERVER_BIN),
        )],
    )
}

fn check_flags(options: &LaunchOptions, api: bool, web: bool) -> Result<Profile, Refusal> {
    let profile = Profile::from_release(options.release);
    if options.test_cluster && profile == Profile::Release {
        return Err(Refusal::Usage(
            "--test-cluster serves the debug cantos-server; drop --release".into(),
        ));
    }
    if !api && (options.test_cluster || options.web_origin.is_some()) {
        return Err(Refusal::Usage(
            "--test-cluster and --web-origin only apply when xtask starts the API".into(),
        ));
    }
    if options.open && !web {
        return Err(Refusal::Usage("--open applies to a --web surface".into()));
    }
    if let Some(origin) = &options.web_origin {
        validate_web_origin(origin)?;
    }
    Ok(profile)
}

pub fn plan_run(
    workspace: &Workspace,
    target: RunTarget,
    options: &LaunchOptions,
) -> Result<LaunchPlan, Refusal> {
    match target {
        RunTarget::CantosApi => {
            if options.surface.is_some() {
                return Err(Refusal::Usage(
                    "cantos-api takes no --web/--mobile flag".into(),
                ));
            }
            let profile = check_flags(options, true, false)?;
            let (requirement, service) = api_service(
                workspace,
                profile,
                options.test_cluster,
                options.web_origin.as_deref(),
            );
            Ok(LaunchPlan {
                requirements: vec![requirement],
                prepare: vec![api_build(workspace, profile)],
                services: vec![service],
                open: None,
            })
        }
        RunTarget::CantosStudio => {
            app_surface(App::Studio, options.surface)?;
            if options.web_origin.is_some() {
                return Err(Refusal::Usage(
                    "run cantos-studio --web sets the API origin to the Trunk server itself".into(),
                ));
            }
            let profile = check_flags(options, true, true)?;
            let origin = App::Studio.web_origin();
            let (requirement, api) =
                api_service(workspace, profile, options.test_cluster, Some(&origin));
            Ok(LaunchPlan {
                requirements: vec![requirement],
                prepare: vec![api_build(workspace, profile)],
                services: vec![api, trunk_service(workspace, App::Studio, profile)],
                open: options.open.then_some(origin),
            })
        }
        RunTarget::Cantos => theatre(workspace, options),
    }
}

pub fn plan_app(
    workspace: &Workspace,
    target: AppTarget,
    options: &LaunchOptions,
) -> Result<LaunchPlan, Refusal> {
    match target {
        AppTarget::CantosStudio => {
            app_surface(App::Studio, options.surface)?;
            let profile = check_flags(options, false, true)?;
            let origin = App::Studio.web_origin();
            Ok(LaunchPlan {
                requirements: vec![Requirement::Listening {
                    name: API_SERVER_BIN,
                    addr: api_addr(),
                    hint: format!(
                        "start it first with `cargo xtask run cantos-api --web-origin {origin}` \
                         (or use `cargo xtask run cantos-studio --web`)"
                    ),
                }],
                prepare: Vec::new(),
                services: vec![trunk_service(workspace, App::Studio, profile)],
                open: options.open.then_some(origin),
            })
        }
        AppTarget::Cantos => theatre(workspace, options),
    }
}

fn theatre(workspace: &Workspace, options: &LaunchOptions) -> Result<LaunchPlan, Refusal> {
    app_surface(App::Theatre, options.surface)?;
    let profile = check_flags(options, false, true)?;
    Ok(LaunchPlan {
        requirements: Vec::new(),
        prepare: Vec::new(),
        services: vec![trunk_service(workspace, App::Theatre, profile)],
        open: options.open.then_some(App::Theatre.web_origin()),
    })
}

/// Fails before any build or process start when a precondition is not met.
fn check_preconditions(
    plan: &LaunchPlan,
    tools: &dyn ToolProbe,
    workspace: &Workspace,
) -> Result<()> {
    let mut problems = Vec::new();
    for requirement in &plan.requirements {
        match requirement {
            Requirement::EnvVar { name, hint } => {
                if std::env::var_os(name).is_none_or(|value| value.is_empty()) {
                    problems.push(format!("{name} is not set: {hint}"));
                }
            }
            Requirement::File { path, hint } => {
                if !path.is_file() {
                    problems.push(format!("{} is missing: {hint}", workspace.relative(path)));
                }
            }
            Requirement::Listening { name, addr, hint } => {
                if !probe::accepts_connections(*addr) {
                    problems.push(format!("{name} is not listening on {addr}: {hint}"));
                }
            }
        }
    }
    for service in &plan.services {
        let missing = tools.missing(&service.tools);
        if !missing.is_empty() {
            let names: Vec<_> = missing.iter().map(|tool| tool.label()).collect();
            problems.push(format!(
                "UNSUPPORTED here: {} needs {} (see `cargo xtask doctor`)",
                service.name,
                names.join(", ")
            ));
        }
        if !probe::port_is_free(service.addr) {
            let alternative = if service.addr == api_addr() {
                "; if it is a Cantos API you started, use `cargo xtask app cantos-studio --web`"
            } else {
                ""
            };
            problems.push(format!(
                "{} ({}) is already in use and xtask does not stop processes it did not start; \
                 free the port{alternative}",
                service.addr, service.name
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        bail!("cannot start:\n  - {}", problems.join("\n  - "))
    }
}

pub fn launch(
    workspace: &Workspace,
    plan: &LaunchPlan,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    check_preconditions(plan, tools, workspace)?;
    if !plan.prepare.is_empty() {
        let report = pipeline::execute("prepare", &plan.prepare, tools, runner);
        if !report.succeeded() {
            report.print();
            return Ok(report.exit_code());
        }
    }

    let mut supervisor = Supervisor::new()?;
    for service in &plan.services {
        if !probe::port_is_free(service.addr) {
            bail!(
                "{} was taken while preparing; xtask does not stop processes it did not start",
                service.addr
            );
        }
        println!(
            "\n==> start {}: {}",
            service.name,
            service.command.display(workspace.root())
        );
        supervisor.spawn(&service.name, &service.command)?;
        if !supervisor.await_ready(service.addr, &service.readiness, service.timeout)? {
            supervisor.shutdown();
            println!("Interrupted before {} became ready.", service.name);
            return Ok(ExitCode::SUCCESS);
        }
        println!("{} ready on http://{}", service.name, service.addr);
    }

    if let Some(url) = &plan.open {
        if let Err(error) = open_browser(url) {
            println!("Could not open a browser ({error:#}); visit {url}");
        }
    }
    println!("\nAll processes are ready. Press Ctrl-C to stop the processes xtask started.");

    let outcome = supervisor.wait()?;
    supervisor.shutdown();
    match outcome {
        Wait::Interrupted => {
            println!("Stopped.");
            Ok(ExitCode::SUCCESS)
        }
        Wait::Exited { name, status } => {
            bail!("{name} exited with {status}; the other processes were stopped")
        }
    }
}

fn open_browser(url: &str) -> Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let status = std::process::Command::new(program).arg(url).status()?;
    if !status.success() {
        bail!("{program} exited with {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::testing::RecordingRunner, tools::testing::FakeTools};

    fn workspace() -> Workspace {
        Workspace::discover().unwrap()
    }

    fn web(options: LaunchOptions) -> LaunchOptions {
        LaunchOptions {
            surface: Some(Surface::Web),
            ..options
        }
    }

    fn env_of(service: &Service, key: &str) -> Option<String> {
        service
            .command
            .env
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn run_api_builds_the_server_and_requires_an_explicit_database() {
        let workspace = workspace();
        let plan = plan_run(&workspace, RunTarget::CantosApi, &LaunchOptions::default()).unwrap();
        assert!(matches!(
            &plan.requirements[..],
            [Requirement::EnvVar {
                name: "DATABASE_URL",
                ..
            }]
        ));
        assert_eq!(
            plan.prepare[0].commands[0].display(workspace.root()),
            "cargo build --locked -p cantos-api --bin cantos-server"
        );
        let api = &plan.services[0];
        assert_eq!(
            api.command.display(workspace.root()),
            "CANTOS_ENV=development CANTOS_BIND_ADDRESS=127.0.0.1:8080 target/debug/cantos-server"
        );
        assert_eq!(api.readiness, Readiness::Tcp);
        assert_eq!(plan.open, None);
    }

    #[test]
    fn run_api_against_the_retained_test_cluster_reuses_the_serving_primitive() {
        let workspace = workspace();
        let options = LaunchOptions {
            test_cluster: true,
            web_origin: Some("http://127.0.0.1:8081".into()),
            ..Default::default()
        };
        let plan = plan_run(&workspace, RunTarget::CantosApi, &options).unwrap();
        assert!(matches!(&plan.requirements[..], [Requirement::File { .. }]));
        assert_eq!(
            plan.services[0].command.display(workspace.root()),
            "CANTOS_WEB_ORIGIN=http://127.0.0.1:8081 python3 scripts/serve_studio.py"
        );
        let release = LaunchOptions {
            release: true,
            ..options
        };
        assert!(matches!(
            plan_run(&workspace, RunTarget::CantosApi, &release),
            Err(Refusal::Usage(_))
        ));
    }

    #[test]
    fn run_studio_web_starts_the_api_for_the_trunk_origin_then_trunk_serve() {
        let workspace = workspace();
        let plan = plan_run(
            &workspace,
            RunTarget::CantosStudio,
            &web(LaunchOptions {
                release: true,
                open: true,
                ..Default::default()
            }),
        )
        .unwrap();
        let [api, studio] = &plan.services[..] else {
            panic!("expected two services");
        };
        assert_eq!(
            env_of(api, "CANTOS_WEB_ORIGIN").as_deref(),
            Some("http://127.0.0.1:8081")
        );
        assert!(api
            .command
            .program
            .ends_with("target/release/cantos-server"));
        assert_eq!(
            studio.command.display(workspace.root()),
            "(in apps/cantos-studio/web) NO_COLOR=true trunk serve --locked --release"
        );
        assert_eq!(studio.addr, loopback(8081));
        assert_eq!(plan.open.as_deref(), Some("http://127.0.0.1:8081"));
    }

    #[test]
    fn app_studio_never_builds_or_starts_the_api() {
        let workspace = workspace();
        let plan = plan_app(
            &workspace,
            AppTarget::CantosStudio,
            &web(Default::default()),
        )
        .unwrap();
        assert!(plan.prepare.is_empty());
        assert_eq!(plan.services.len(), 1);
        assert!(matches!(
            &plan.requirements[..],
            [Requirement::Listening { addr, .. }] if *addr == loopback(8080)
        ));
    }

    #[test]
    fn theatre_has_only_its_trunk_server() {
        let workspace = workspace();
        for plan in [
            plan_run(&workspace, RunTarget::Cantos, &web(Default::default())).unwrap(),
            plan_app(&workspace, AppTarget::Cantos, &web(Default::default())).unwrap(),
        ] {
            assert!(plan.requirements.is_empty());
            assert_eq!(plan.services.len(), 1);
            assert_eq!(plan.services[0].addr, loopback(8082));
        }
    }

    #[test]
    fn refuses_mobile_missing_surfaces_and_misplaced_flags() {
        let workspace = workspace();
        let mobile = LaunchOptions {
            surface: Some(Surface::Mobile),
            ..Default::default()
        };
        assert!(matches!(
            plan_run(&workspace, RunTarget::CantosStudio, &mobile),
            Err(Refusal::Unsupported { .. })
        ));
        assert!(matches!(
            plan_app(&workspace, AppTarget::Cantos, &mobile),
            Err(Refusal::Unsupported { .. })
        ));
        assert!(matches!(
            plan_run(&workspace, RunTarget::Cantos, &LaunchOptions::default()),
            Err(Refusal::Usage(_))
        ));
        let cluster_for_theatre = web(LaunchOptions {
            test_cluster: true,
            ..Default::default()
        });
        assert!(matches!(
            plan_run(&workspace, RunTarget::Cantos, &cluster_for_theatre),
            Err(Refusal::Usage(_))
        ));
        let open_api = LaunchOptions {
            open: true,
            ..Default::default()
        };
        assert!(matches!(
            plan_run(&workspace, RunTarget::CantosApi, &open_api),
            Err(Refusal::Usage(_))
        ));
    }

    #[test]
    fn web_origins_must_be_loopback_http() {
        assert!(validate_web_origin("http://127.0.0.1:8081").is_ok());
        assert!(validate_web_origin("http://localhost:3000").is_ok());
        for bad in [
            "https://127.0.0.1:8081",
            "http://example.com:80",
            "http://127.0.0.1",
            "http://127.0.0.1:8081/",
            "http://127.0.0.1:99999",
        ] {
            assert!(validate_web_origin(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_taken_port_or_missing_tool_fails_before_anything_runs() {
        let workspace = workspace();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut plan = plan_run(&workspace, RunTarget::Cantos, &web(Default::default())).unwrap();
        plan.services[0].addr = listener.local_addr().unwrap();
        let mut runner = RecordingRunner::default();
        let error = launch(
            &workspace,
            &plan,
            &FakeTools::without(&[Tool::Trunk]),
            &mut runner,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("does not stop processes"), "{error}");
        assert!(!error.contains("app cantos-studio"), "{error}");
        assert!(error.contains("UNSUPPORTED here"), "{error}");
        assert!(runner.commands.is_empty());
    }
}
