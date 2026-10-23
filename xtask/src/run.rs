//! `run` and `app`: local process plans and their supervised execution.
//!
//! `run` builds what the target needs and starts the native development composition: one
//! `cantos-identity` and one `cantos-server`, each owning its own embedded Turso file under
//! a private development data directory (see [`crate::native_dev`]), plus Trunk for a Web
//! surface. Schema creation is the explicit `cantos-migrate` / `cantos-identity migrate` steps
//! executed here before anything serves. `app` starts only Web: Studio requires an existing
//! API; Theatre warns when its catalog dependency is unavailable so explicit offline previews
//! remain usable.
//! Theatre `run --web` also owns an isolated installed Auth Emulator. Native proof confirmation stays in Identity; no cloud resource, database server, container or object store is provisioned.

use std::{io::IsTerminal, net::SocketAddr, process::ExitCode, time::Duration};

use anyhow::{bail, Context, Result};
use clap::ValueEnum;

use crate::{
    auth_emulator,
    build::{self, app_surface, Profile, Refusal, Surface},
    command::{CommandSpec, Runner},
    config::{loopback, App, API_PORT, API_SERVER_BIN, IDENTITY_BIN},
    native_dev::{self, Layout, WithIdentity},
    pipeline::{self, Step},
    probe::{self, Endpoint, Readiness},
    process::{Supervisor, Wait},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

const API_READY_TIMEOUT: Duration = Duration::from_secs(60);
const IDENTITY_READY_TIMEOUT: Duration = Duration::from_secs(60);
/// Trunk compiles the WASM bundle before it serves the first page.
const TRUNK_READY_TIMEOUT: Duration = Duration::from_secs(900);

#[allow(clippy::enum_variant_names)] // CLI names mirror the package names
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RunTarget {
    /// The native Server and Identity in the foreground.
    CantosServer,
    /// Cantos Studio: with --web, the native Server + Identity plus a `trunk serve` frontend.
    CantosStudio,
    /// Cantos Theatre: with --web, the native Server + Identity plus a `trunk serve` frontend.
    #[value(alias = "cantos")]
    CantosTheatre,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AppTarget {
    /// Cantos Studio (creator client).
    CantosStudio,
    /// Cantos Theatre (listener client).
    #[value(alias = "cantos")]
    CantosTheatre,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    pub surface: Option<Surface>,
    pub release: bool,
    pub open: bool,
    pub web_origin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requirement {
    /// Another process must already accept connections here.
    Listening {
        name: &'static str,
        addr: SocketAddr,
        hint: String,
    },
    /// Frontend-only Theatre can still serve explicit offline previews when this fails.
    TheatreApi { addr: SocketAddr, hint: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub command: CommandSpec,
    pub tools: Vec<Tool>,
    pub endpoint: Endpoint,
    pub readiness: Readiness,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// Theatre's supervised local verification action provider.
    pub firebase_auth_emulator: bool,
    pub requirements: Vec<Requirement>,
    /// The development data directories xtask creates (0700) before anything runs.
    pub composition: Option<Layout>,
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

/// The builds, then the two explicit migrations. Nothing here starts a serving process, and a
/// failed migration stops the run before any process is spawned.
pub fn native_prepare(workspace: &Workspace, layout: &Layout, profile: Profile) -> Vec<Step> {
    vec![
        Step::new(
            format!("build cantos-server binaries ({})", profile.label()),
            &[Tool::Cargo],
            vec![
                build::api_binaries(workspace, profile, Some(API_SERVER_BIN)),
                build::api_binaries(workspace, profile, Some(crate::config::SERVER_MIGRATE_BIN)),
            ],
        ),
        Step::new(
            format!(
                "build {IDENTITY_BIN} --features service ({})",
                profile.label()
            ),
            &[Tool::Cargo],
            vec![build::identity_binary(workspace, profile)
                .args(["--features", "local-development"])],
        ),
        Step::new(
            "migrate server.db (cantos-migrate)",
            &[],
            vec![native_dev::migrate_server(workspace, layout, profile)],
        ),
        Step::new(
            "migrate identity.db (cantos-identity migrate)",
            &[],
            vec![native_dev::migrate_identity(workspace, layout, profile)],
        ),
    ]
}

/// Identity first (the Server dials its socket lazily, but an Identity that fails to start
/// should stop the run before the Server reports ready), then the Server.
fn native_services(
    workspace: &Workspace,
    layout: &Layout,
    profile: Profile,
    web_origin: Option<&str>,
) -> Result<Vec<Service>, Refusal> {
    let signup_link = format!("{}/", App::Theatre.web_origin());
    let identity = Service {
        name: IDENTITY_BIN.into(),
        command: native_dev::identity_serve(workspace, layout, profile, Some(&signup_link))
            .env("CANTOS_IDENTITY_LOCAL_DEVELOPMENT", "1")
            .env("CANTOS_IDENTITY_MAIL_PROVIDER", "disabled"),
        tools: Vec::new(),
        endpoint: Endpoint::Unix(layout.socket.clone()),
        readiness: Readiness::Tcp,
        timeout: IDENTITY_READY_TIMEOUT,
    };
    let media_origin = std::env::var("CANTOS_LOCAL_MEDIA_ORIGIN").ok();
    let mut command = native_dev::server_serve(
        workspace,
        layout,
        profile,
        &web_origin
            .map(str::to_owned)
            .unwrap_or_else(|| App::Studio.web_origin()),
        WithIdentity::Yes,
    );
    for (name, value) in native_dev::media_environment(layout, media_origin.as_deref())? {
        command = command.env(name, value);
    }
    let server = Service {
        name: API_SERVER_BIN.into(),
        command,
        tools: Vec::new(),
        endpoint: Endpoint::Tcp(api_addr()),
        readiness: Readiness::Tcp,
        timeout: API_READY_TIMEOUT,
    };
    Ok(vec![identity, server])
}

fn trunk_service(workspace: &Workspace, app: App, profile: Profile) -> Service {
    Service {
        name: format!("{} web (trunk serve)", app.cli_name()),
        command: match app {
            // Local integration must never inherit Theatre's production default.
            App::Theatre => build::trunk(workspace, app, "serve", profile)
                .env("CANTOS_API_ORIGIN", "http://127.0.0.1:8080"),
            App::Studio => build::trunk(workspace, app, "serve", profile),
        },
        tools: vec![Tool::Cargo, Tool::Wasm32Target, Tool::Trunk],
        endpoint: Endpoint::Tcp(loopback(app.web_port())),
        readiness: Readiness::Http {
            path: "/",
            marker: app.web_marker(),
        },
        timeout: TRUNK_READY_TIMEOUT,
    }
}

fn check_flags(options: &LaunchOptions, api: bool, web: bool) -> Result<Profile, Refusal> {
    let profile = Profile::from_release(options.release);
    if !api && options.web_origin.is_some() {
        return Err(Refusal::Usage(
            "--web-origin only applies when xtask starts the Server".into(),
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
        RunTarget::CantosServer => {
            if options.surface.is_some() {
                return Err(Refusal::Usage(
                    "cantos-server takes no --web/--mobile flag".into(),
                ));
            }
            let profile = check_flags(options, true, false)?;
            let layout = Layout::for_current_user(workspace)?;
            Ok(LaunchPlan {
                firebase_auth_emulator: false,
                requirements: Vec::new(),
                prepare: native_prepare(workspace, &layout, profile),
                services: native_services(
                    workspace,
                    &layout,
                    profile,
                    options.web_origin.as_deref(),
                )?,
                composition: Some(layout),
                open: None,
            })
        }
        RunTarget::CantosStudio => run_web(workspace, App::Studio, options),
        RunTarget::CantosTheatre => run_web(workspace, App::Theatre, options),
    }
}

fn run_web(
    workspace: &Workspace,
    app: App,
    options: &LaunchOptions,
) -> Result<LaunchPlan, Refusal> {
    app_surface(app, options.surface)?;
    let profile = check_flags(options, true, true)?;
    let origin = app.web_origin();
    if options
        .web_origin
        .as_ref()
        .is_some_and(|value| value != &origin)
    {
        return Err(Refusal::Usage(format!(
            "run {} --web requires --web-origin {origin}, matching its Trunk server; \
             omit --web-origin to use the default per-surface origins",
            app.cli_name()
        )));
    }
    let layout = Layout::for_current_user(workspace)?;
    let mut services = native_services(workspace, &layout, profile, None)?;
    let mut frontend = trunk_service(workspace, app, profile);
    if app == App::Theatre {
        let api = services
            .iter_mut()
            .find(|service| service.name == API_SERVER_BIN)
            .expect("the composition always contains the Server");
        api.readiness = Readiness::TheatreCatalog;
        frontend.readiness = Readiness::TheatreWeb;
    }
    services.push(frontend);
    Ok(LaunchPlan {
        firebase_auth_emulator: app == App::Theatre,
        requirements: Vec::new(),
        prepare: native_prepare(workspace, &layout, profile),
        composition: Some(layout),
        services,
        open: options.open.then_some(origin),
    })
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
                firebase_auth_emulator: false,
                requirements: vec![Requirement::Listening {
                    name: API_SERVER_BIN,
                    addr: api_addr(),
                    hint: format!(
                        "start it first with `cargo xtask run cantos-server --web-origin {origin}` \
                         (or use `cargo xtask run cantos-studio --web`)"
                    ),
                }],
                composition: None,
                prepare: Vec::new(),
                services: vec![trunk_service(workspace, App::Studio, profile)],
                open: options.open.then_some(origin),
            })
        }
        AppTarget::CantosTheatre => theatre(workspace, options),
    }
}

fn theatre(workspace: &Workspace, options: &LaunchOptions) -> Result<LaunchPlan, Refusal> {
    app_surface(App::Theatre, options.surface)?;
    let profile = check_flags(options, false, true)?;
    Ok(LaunchPlan {
        firebase_auth_emulator: false,
        requirements: vec![Requirement::TheatreApi {
            addr: api_addr(),
            hint: "start `cargo xtask run cantos-server`, or use \
                   `cargo xtask run cantos-theatre --web`; \
                   explicit ?mode=mock or ?mode=membership-preview remain offline previews"
                .into(),
        }],
        composition: None,
        prepare: Vec::new(),
        services: vec![trunk_service(workspace, App::Theatre, profile)],
        open: options.open.then_some(App::Theatre.web_origin()),
    })
}

/// Fails before any build or process start when a precondition is not met.
fn check_preconditions(plan: &LaunchPlan, tools: &dyn ToolProbe) -> Result<()> {
    let mut problems = Vec::new();
    for requirement in &plan.requirements {
        match requirement {
            Requirement::Listening { name, addr, hint } => {
                if !probe::accepts_connections(*addr) {
                    problems.push(format!("{name} is not listening on {addr}: {hint}"));
                }
            }
            Requirement::TheatreApi { addr, hint } => {
                let observed = probe::probe(*addr, &Readiness::TheatreCatalog);
                match observed {
                    probe::Probe::Ready => println!(
                        "Theatre API anonymous catalog reachable at http://{addr}/api/v1/theatre/works?limit=20"
                    ),
                    _ => {
                        let warning = format!(
                            "WARNING: FRONTEND ONLY. Theatre API catalog is unavailable at http://{addr} \
                             ({observed:?}). Live Home is not ready. Next: {hint}"
                        );
                        if std::io::stderr().is_terminal() {
                            eprintln!("\x1b[31m{warning}\x1b[0m");
                        } else {
                            eprintln!("{warning}");
                        }
                    }
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
        match &service.endpoint {
            Endpoint::Tcp(addr) => {
                if !probe::port_is_free(*addr) {
                    let alternative = if *addr == api_addr() {
                        "; if it is a Cantos Server you started, use `cargo xtask app <cantos-studio|cantos-theatre> --web`"
                    } else {
                        ""
                    };
                    problems.push(format!(
                        "{addr} ({}) is already in use and xtask does not stop processes it did not start; \
                         free the port{alternative}",
                        service.name
                    ));
                }
            }
            Endpoint::Unix(path) => {
                if probe::accepts_unix(path) {
                    problems.push(format!(
                        "{} ({}) already answers and xtask does not stop processes it did not start; \
                         stop that Identity first",
                        path.display(),
                        service.name
                    ));
                }
            }
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
    // All native local runs refuse ambient cloud/mail configuration, including Server-only.
    if plan.composition.is_some() {
        auth_emulator::reject_inherited_configuration(
            std::env::vars_os().filter_map(|(name, _)| name.into_string().ok()),
        )?;
    }
    let installed_emulator = plan
        .firebase_auth_emulator
        .then(auth_emulator::Installed::inspect)
        .transpose()?;
    check_preconditions(plan, tools)?;
    if let Some(layout) = &plan.composition {
        native_dev::ensure_private_directories(layout)?;
        println!(
            "Development data (private, kept between runs): {}\n  server.db   {}\n  identity.db {}\n  identity socket {}",
            workspace.relative(&layout.root),
            workspace.relative(&layout.server_db),
            workspace.relative(&layout.identity_db),
            layout.socket.display()
        );
    }
    if !plan.prepare.is_empty() {
        let report = pipeline::execute("prepare", &plan.prepare, tools, runner);
        if !report.succeeded() {
            report.print();
            return Ok(report.exit_code());
        }
    }

    // The run directory must drop after the supervisor, including on an early error.
    let mut emulator_run = None;
    let mut supervisor = Supervisor::new()?;
    if let Some(installed) = installed_emulator {
        emulator_run = Some(auth_emulator::Run::start(
            workspace,
            &installed,
            &mut supervisor,
        )?);
    }
    if let Some(emulator) = &emulator_run {
        println!("Firebase Auth Emulator {version}: loopback {address}, project {project}.\n  Local developer action inventory: http://{address}/emulator/v1/projects/{project}/oobCodes\n  Verification actions stay local; native password login remains a separate step.",
            version = auth_emulator::VERSION, address = emulator.address, project = emulator.project);
    }
    for service in &plan.services {
        let mut command = service.command.clone();
        if service.name == IDENTITY_BIN {
            if let Some(emulator) = &emulator_run {
                command
                    .env
                    .retain(|(name, _)| name != "CANTOS_IDENTITY_MAIL_PROVIDER");
                command = command
                    .env("CANTOS_IDENTITY_MAIL_PROVIDER", "firebase-emulator")
                    .env("CANTOS_IDENTITY_FIREBASE_PROJECT", &emulator.project)
                    .env(
                        "CANTOS_IDENTITY_FIREBASE_EMULATOR_ENDPOINT",
                        emulator.address.to_string(),
                    )
                    .env(
                        "CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE",
                        emulator.key_file.display().to_string(),
                    )
                    .env(
                        "CANTOS_IDENTITY_FIREBASE_ACTION_ORIGIN",
                        App::Theatre.web_origin(),
                    )
                    .env(
                        "CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER",
                        format!("{}/", App::Theatre.web_origin()),
                    );
            }
        }
        let free = match &service.endpoint {
            Endpoint::Tcp(addr) => probe::port_is_free(*addr),
            Endpoint::Unix(path) => !probe::accepts_unix(path),
        };
        if !free {
            bail!(
                "{} was taken while preparing; xtask does not stop processes it did not start",
                service.endpoint
            );
        }
        println!(
            "\n==> start {}: {}",
            service.name,
            command.display(workspace.root())
        );
        supervisor.spawn(&service.name, &command)?;
        let ready = supervisor
            .await_endpoint(&service.endpoint, &service.readiness, service.timeout)
            .with_context(|| readiness_hint(service))?;
        if !ready {
            supervisor.shutdown();
            println!("Interrupted before {} became ready.", service.name);
            return Ok(ExitCode::SUCCESS);
        }
        match &service.endpoint {
            Endpoint::Tcp(addr) => println!(
                "{} ready on http://{addr} ({:?})",
                service.name, service.readiness
            ),
            Endpoint::Unix(path) => println!(
                "{} accepts connections on {} (a listening process, not an authentication)",
                service.name,
                path.display()
            ),
        }
    }

    if let Some(url) = &plan.open {
        if let Err(error) = open_browser(url) {
            println!("Could not open a browser ({error:#}); visit {url}");
        }
    }
    println!(
        "\n{} Press Ctrl-C to stop the processes xtask started.",
        ready_summary(plan)
    );

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

fn readiness_hint(service: &Service) -> &'static str {
    if matches!(service.endpoint, Endpoint::Unix(_)) {
        return "Identity startup failed: check its output, that `cantos-identity migrate` succeeded, and that no other process owns identity.db. Local mail is disabled until the Firebase/native proof bridge exists; inherited production mail configuration is refused.";
    }
    match service.readiness {
        Readiness::TheatreCatalog => "Theatre Server startup failed: check the Server output, that `cantos-migrate` succeeded, and that no other process (an operator command, an old Server) owns server.db. No database server or identity provider is provisioned; no mock fallback is selected.",
        Readiness::TheatreWeb => "Theatre Web startup failed: check Trunk output and GET /api/v1/theatre/works?limit=20 at both http://127.0.0.1:8080 and http://127.0.0.1:8082. The API and proxy must return JSON; /session success is optional.",
        _ => "Local process startup failed; inspect the process output and the private development files.",
    }
}

fn ready_summary(plan: &LaunchPlan) -> &'static str {
    if plan
        .services
        .iter()
        .any(|service| service.name == API_SERVER_BIN)
    {
        "All processes are ready."
    } else {
        "Frontend is ready; backend availability and live integration are separate."
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

    fn server(plan: &LaunchPlan) -> &Service {
        plan.services
            .iter()
            .find(|service| service.name == API_SERVER_BIN)
            .unwrap()
    }

    #[test]
    fn run_builds_both_executables_and_migrates_each_file_before_anything_serves() {
        let workspace = workspace();
        let plan = plan_run(
            &workspace,
            RunTarget::CantosServer,
            &LaunchOptions::default(),
        )
        .unwrap();
        assert!(plan.requirements.is_empty(), "no DATABASE_URL, no cluster");
        let lines: Vec<String> = plan
            .prepare
            .iter()
            .flat_map(|step| step.commands.iter())
            .map(|command| command.display(workspace.root()))
            .collect();
        let layout = plan.composition.as_ref().unwrap();
        assert_eq!(
            lines,
            [
                "cargo build --locked -p cantos-server --bin cantos-server".to_owned(),
                "cargo build --locked -p cantos-server --bin cantos-migrate".to_owned(),
                "cargo build --locked -p cantos-identity --features service --bin cantos-identity --features local-development"
                    .to_owned(),
                format!(
                    "CANTOS_SERVER_DATABASE={} target/debug/cantos-migrate",
                    layout.server_db.display()
                ),
                format!(
                    "CANTOS_IDENTITY_DATABASE={} target/debug/cantos-identity migrate",
                    layout.identity_db.display()
                ),
            ]
        );
        // Builds first, then migrations, and no serving command is a preparation step.
        assert!(plan
            .prepare
            .iter()
            .flat_map(|step| step.commands.iter())
            .all(|command| !command.args.contains(&"serve".to_owned())));
    }

    #[test]
    fn identity_starts_before_the_server_and_each_has_its_own_endpoint_and_file() {
        let plan = plan_run(
            &workspace(),
            RunTarget::CantosServer,
            &LaunchOptions::default(),
        )
        .unwrap();
        let layout = plan.composition.clone().unwrap();
        let [identity, server] = &plan.services[..] else {
            panic!("expected Identity then Server");
        };
        assert_eq!(identity.name, IDENTITY_BIN);
        assert_eq!(identity.endpoint, Endpoint::Unix(layout.socket.clone()));
        assert_eq!(identity.readiness, Readiness::Tcp);
        assert_eq!(server.endpoint, Endpoint::Tcp(loopback(8080)));
        assert_eq!(server.readiness, Readiness::Tcp);
        assert_eq!(
            env_of(identity, "CANTOS_IDENTITY_DATABASE").unwrap(),
            layout.identity_db.display().to_string()
        );
        assert_eq!(
            env_of(server, "CANTOS_SERVER_DATABASE").unwrap(),
            layout.server_db.display().to_string()
        );
        assert_ne!(
            env_of(identity, "CANTOS_IDENTITY_DATABASE"),
            env_of(server, "CANTOS_SERVER_DATABASE")
        );
        // The Server dials the socket Identity serves, as the uid Identity runs as.
        assert_eq!(
            env_of(server, "CANTOS_IDENTITY_SOCKET"),
            env_of(identity, "CANTOS_IDENTITY_SOCKET")
        );
        assert_eq!(
            env_of(server, "CANTOS_IDENTITY_PEER_UID"),
            env_of(identity, "CANTOS_IDENTITY_SERVER_UIDS")
        );
        assert_eq!(plan.open, None);
    }

    #[test]
    fn theatre_selects_local_emulator_and_disables_delivery_at_the_actual_theatre_origin() {
        let plan = plan_run(
            &workspace(),
            RunTarget::CantosTheatre,
            &web(LaunchOptions::default()),
        )
        .unwrap();
        assert!(plan.firebase_auth_emulator);
        let identity = plan
            .services
            .iter()
            .find(|service| service.name == IDENTITY_BIN)
            .unwrap();
        assert_eq!(
            env_of(identity, "CANTOS_IDENTITY_MAIL_PROVIDER").as_deref(),
            Some("disabled")
        );
        assert_eq!(
            env_of(identity, "CANTOS_IDENTITY_LOCAL_DEVELOPMENT").as_deref(),
            Some("1")
        );
        assert_eq!(
            env_of(identity, "CANTOS_IDENTITY_SIGNUP_LINK_BASE"),
            Some(format!("{}/", App::Theatre.web_origin()))
        );
        let app = plan_app(
            &workspace(),
            AppTarget::CantosTheatre,
            &web(LaunchOptions::default()),
        )
        .unwrap();
        assert!(
            !app.firebase_auth_emulator,
            "frontend-only attachment owns no infrastructure"
        );
    }

    #[test]
    fn no_plan_mentions_a_database_url_a_cluster_a_container_or_a_provider() {
        let workspace = workspace();
        let plans = [
            plan_run(
                &workspace,
                RunTarget::CantosServer,
                &LaunchOptions::default(),
            )
            .unwrap(),
            plan_run(
                &workspace,
                RunTarget::CantosStudio,
                &web(LaunchOptions::default()),
            )
            .unwrap(),
            plan_run(
                &workspace,
                RunTarget::CantosTheatre,
                &web(LaunchOptions::default()),
            )
            .unwrap(),
        ];
        for plan in plans {
            let text: String = plan
                .prepare
                .iter()
                .flat_map(|step| step.commands.iter())
                .chain(plan.services.iter().map(|service| &service.command))
                .map(|command| command.display(workspace.root()))
                .collect::<Vec<_>>()
                .join("\n");
            for retired in [
                "DATABASE_URL",
                "postgres",
                "podman",
                "docker",
                "kanidm",
                "OIDC",
                "test-cluster",
                "serve_studio",
            ] {
                assert!(!text.contains(retired), "{retired}: {text}");
            }
            assert!(plan.services.iter().all(|service| service
                .tools
                .iter()
                .all(|tool| !matches!(tool, Tool::Python3))));
        }
    }

    #[test]
    fn every_run_owns_one_server_with_both_web_policies_and_apps_attach_without_a_server() {
        for target in [
            RunTarget::CantosServer,
            RunTarget::CantosStudio,
            RunTarget::CantosTheatre,
        ] {
            let options = if target == RunTarget::CantosServer {
                LaunchOptions::default()
            } else {
                web(LaunchOptions::default())
            };
            let plan = plan_run(&workspace(), target, &options).unwrap();
            let servers: Vec<_> = plan
                .services
                .iter()
                .filter(|service| service.name == API_SERVER_BIN)
                .collect();
            assert_eq!(servers.len(), 1);
            let identities = plan
                .services
                .iter()
                .filter(|service| service.name == IDENTITY_BIN)
                .count();
            assert_eq!(identities, 1);
            let server = servers[0];
            assert_eq!(server.endpoint, Endpoint::Tcp(loopback(8080)));
            assert_eq!(
                env_of(server, "CANTOS_STUDIO_WEB_ORIGIN"),
                Some(App::Studio.web_origin())
            );
            assert_eq!(
                env_of(server, "CANTOS_THEATRE_WEB_ORIGIN"),
                Some(App::Theatre.web_origin())
            );
            assert!(server
                .command
                .env_remove
                .iter()
                .any(|name| name == "CANTOS_BROWSER_API_SURFACE"));
        }
        for target in [AppTarget::CantosStudio, AppTarget::CantosTheatre] {
            let plan = plan_app(&workspace(), target, &web(LaunchOptions::default())).unwrap();
            assert!(plan.composition.is_none() && plan.prepare.is_empty());
            assert!(plan
                .services
                .iter()
                .all(|service| service.name != API_SERVER_BIN && service.name != IDENTITY_BIN));
        }
    }

    #[test]
    fn standalone_server_may_pin_studios_origin_and_release_uses_release_binaries() {
        let workspace = workspace();
        let plan = plan_run(
            &workspace,
            RunTarget::CantosServer,
            &LaunchOptions {
                release: true,
                web_origin: Some("http://127.0.0.1:8081".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(server(&plan)
            .command
            .program
            .ends_with("target/release/cantos-server"));
        assert!(plan.services[0]
            .command
            .program
            .ends_with("target/release/cantos-identity"));
        assert_eq!(
            env_of(server(&plan), "CANTOS_STUDIO_WEB_ORIGIN").as_deref(),
            Some("http://127.0.0.1:8081")
        );
    }

    #[test]
    fn run_studio_web_starts_identity_and_the_server_then_trunk_serve() {
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
        let [identity, api, studio] = &plan.services[..] else {
            panic!("expected three services");
        };
        assert_eq!(identity.name, IDENTITY_BIN);
        assert!(api
            .command
            .program
            .ends_with("target/release/cantos-server"));
        assert_eq!(
            studio.command.display(workspace.root()),
            "(in apps/cantos-studio/web) NO_COLOR=true trunk serve --locked --release"
        );
        assert_eq!(studio.endpoint, Endpoint::Tcp(loopback(8081)));
        assert_eq!(plan.open.as_deref(), Some("http://127.0.0.1:8081"));
    }

    #[test]
    fn app_studio_never_builds_or_starts_a_native_process() {
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
    fn theatre_run_starts_identity_then_the_server_before_trunk() {
        let plan = plan_run(
            &workspace(),
            RunTarget::CantosTheatre,
            &web(Default::default()),
        )
        .unwrap();
        assert_eq!(plan.services.len(), 3);
        assert_eq!(plan.services[0].name, IDENTITY_BIN);
        assert_eq!(plan.services[1].endpoint, Endpoint::Tcp(loopback(8080)));
        assert_eq!(plan.services[2].endpoint, Endpoint::Tcp(loopback(8082)));
        assert_eq!(
            env_of(&plan.services[2], "CANTOS_API_ORIGIN").as_deref(),
            Some("http://127.0.0.1:8080")
        );
        assert!(plan.requirements.is_empty());
        assert_eq!(
            env_of(&plan.services[1], "CANTOS_THEATRE_WEB_ORIGIN").as_deref(),
            Some("http://127.0.0.1:8082")
        );
        assert_eq!(plan.prepare.len(), 4);
    }

    #[test]
    fn theatre_app_preserves_offline_previews_without_claiming_integrated_readiness() {
        let plan = plan_app(
            &workspace(),
            AppTarget::CantosTheatre,
            &web(Default::default()),
        )
        .unwrap();
        assert!(plan.prepare.is_empty());
        assert_eq!(plan.services.len(), 1);
        assert_eq!(plan.services[0].endpoint, Endpoint::Tcp(loopback(8082)));
        assert!(
            matches!(&plan.requirements[..], [Requirement::TheatreApi { addr, .. }] if *addr == loopback(8080))
        );
        assert_eq!(
            plan.services[0].readiness,
            Readiness::Http {
                path: "/",
                marker: "<title>Cantos</title>"
            }
        );
        assert_eq!(
            ready_summary(&plan),
            "Frontend is ready; backend availability and live integration are separate."
        );
    }

    #[test]
    fn full_stack_theatre_checks_anonymous_catalog_directly_and_through_proxy() {
        let plan = plan_run(
            &workspace(),
            RunTarget::CantosTheatre,
            &web(Default::default()),
        )
        .unwrap();
        assert_eq!(server(&plan).readiness, Readiness::TheatreCatalog);
        assert_eq!(plan.services[2].readiness, Readiness::TheatreWeb);
        assert_eq!(plan.services[0].readiness, Readiness::Tcp);
        assert_eq!(ready_summary(&plan), "All processes are ready.");
    }

    #[test]
    fn web_profile_origin_must_match_its_actual_trunk_origin() {
        for (target, origin) in [
            (RunTarget::CantosStudio, "http://127.0.0.1:8081"),
            (RunTarget::CantosTheatre, "http://127.0.0.1:8082"),
        ] {
            let options = web(LaunchOptions {
                web_origin: Some(origin.into()),
                ..Default::default()
            });
            let plan = plan_run(&workspace(), target, &options).unwrap();
            assert_eq!(
                env_of(
                    server(&plan),
                    if matches!(target, RunTarget::CantosStudio) {
                        "CANTOS_STUDIO_WEB_ORIGIN"
                    } else {
                        "CANTOS_THEATRE_WEB_ORIGIN"
                    }
                )
                .as_deref(),
                Some(origin)
            );
            let wrong = web(LaunchOptions {
                web_origin: Some("http://localhost:3000".into()),
                ..Default::default()
            });
            assert!(
                matches!(plan_run(&workspace(), target, &wrong), Err(Refusal::Usage(message)) if message.contains("matching its Trunk server"))
            );
        }
    }

    #[test]
    fn unavailable_advisory_catalog_does_not_block_frontend_preview() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let plan = LaunchPlan {
            firebase_auth_emulator: false,
            requirements: vec![Requirement::TheatreApi {
                addr,
                hint: "start the Server explicitly".into(),
            }],
            composition: None,
            prepare: Vec::new(),
            services: Vec::new(),
            open: None,
        };
        check_preconditions(&plan, &FakeTools::all()).unwrap();
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
            Err(Refusal::Usage(message)) if message == "Cantos Studio is web-only; --mobile is out of scope"
        ));
        assert!(matches!(
            plan_app(&workspace, AppTarget::CantosStudio, &mobile),
            Err(Refusal::Usage(message)) if message == "Cantos Studio is web-only; --mobile is out of scope"
        ));
        assert!(matches!(
            plan_app(&workspace, AppTarget::CantosTheatre, &mobile),
            Err(Refusal::Unsupported { .. })
        ));
        assert!(matches!(
            plan_run(
                &workspace,
                RunTarget::CantosTheatre,
                &LaunchOptions::default()
            ),
            Err(Refusal::Usage(_))
        ));
        let open_api = LaunchOptions {
            open: true,
            ..Default::default()
        };
        assert!(matches!(
            plan_run(&workspace, RunTarget::CantosServer, &open_api),
            Err(Refusal::Usage(_))
        ));
        let origin_for_app = web(LaunchOptions {
            web_origin: Some("http://127.0.0.1:8081".into()),
            ..Default::default()
        });
        assert!(matches!(
            plan_app(&workspace, AppTarget::CantosStudio, &origin_for_app),
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
    fn a_taken_port_or_missing_tool_fails_before_any_directory_or_command() {
        let workspace = workspace();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut plan = plan_run(
            &workspace,
            RunTarget::CantosTheatre,
            &web(Default::default()),
        )
        .unwrap();
        plan.composition = None;
        // The installed Auth Emulator is inspected on the real host; keep this plan hermetic.
        plan.firebase_auth_emulator = false;
        plan.services[1].endpoint = Endpoint::Tcp(listener.local_addr().unwrap());
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

    #[cfg(unix)]
    #[test]
    fn an_identity_socket_that_already_answers_is_not_ours_to_replace() {
        let dir = std::env::temp_dir().join(format!("xtask-run-uds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("identity.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let mut plan = plan_run(
            &workspace(),
            RunTarget::CantosServer,
            &LaunchOptions::default(),
        )
        .unwrap();
        plan.composition = None;
        plan.services.truncate(1);
        plan.services[0].endpoint = Endpoint::Unix(path);
        let mut runner = RecordingRunner::default();
        let error = launch(&workspace(), &plan, &FakeTools::all(), &mut runner)
            .unwrap_err()
            .to_string();
        assert!(error.contains("already answers"), "{error}");
        assert!(runner.commands.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_migration_stops_the_run_before_any_process_is_spawned() {
        let workspace = workspace();
        let plan = plan_run(
            &workspace,
            RunTarget::CantosServer,
            &LaunchOptions::default(),
        )
        .unwrap();
        let mut runner = RecordingRunner {
            fail_on: Some("target/debug/cantos-migrate".into()),
            ..Default::default()
        };
        // Preconditions hold, directories are not touched by this fixture, and the run ends
        // at the failing step with a failure exit before `Supervisor` could spawn anything.
        let mut plan = plan;
        plan.composition = None;
        plan.services.clear();
        let code = launch(&workspace, &plan, &FakeTools::all(), &mut runner).unwrap();
        assert_ne!(code, ExitCode::SUCCESS);
        let last = runner.commands.last().unwrap();
        assert!(last.program.ends_with("cantos-migrate"));
        // The identity migration never ran after the Server's failed.
        assert!(runner
            .commands
            .iter()
            .all(|command| !command.args.contains(&"migrate".to_owned())));
    }
}
