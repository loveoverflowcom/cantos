//! Build plans: which Cargo or Trunk invocation produces each target, per profile.
//!
//! `dev` is the default unoptimized local build. `--release` is an optimized local artifact
//! (`cargo build --release`, `trunk build --release`); it is not a production image, a signed
//! bundle or a deployment, none of which exist yet.

use std::path::Path;

use clap::ValueEnum;

use crate::{
    command::CommandSpec,
    config::{
        App, API_PACKAGE, FORGE_PACKAGE, IDENTITY_BIN, IDENTITY_PACKAGE, IDENTITY_SERVICE_FEATURE,
        THEATRE_MOBILE_DIR,
    },
    pipeline::Step,
    tools::Tool,
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BuildTarget {
    /// The Axum backend (`cantos-server`) and its offline `cantos-migrate`/`cantos-operator` CLIs
    /// (`services/cantos-server`); it owns one embedded Turso file, `server.db`.
    CantosServer,
    /// The native Identity executable with its `service` feature (`services/cantos-identity`);
    /// it owns one embedded Turso file, `identity.db`.
    CantosIdentity,
    /// Cantos Forge, the producer-side CLI (`services/cantos-forge-functions`); runs outside the API.
    CantosForge,
    /// Cantos Studio, the creator product (`apps/cantos-studio`).
    CantosStudio,
    /// Cantos Theatre, the listener product (`apps/cantos-theatre`).
    #[value(alias = "cantos")]
    CantosTheatre,
    /// The Server, Identity, the Forge CLI and every implemented Web client.
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Web,
    Mobile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Dev,
    Release,
}

impl Profile {
    pub fn from_release(release: bool) -> Self {
        if release {
            Profile::Release
        } else {
            Profile::Dev
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Profile::Dev => "dev",
            Profile::Release => "release",
        }
    }

    /// Cargo's output directory name under `target/`.
    pub fn cargo_dir(self) -> &'static str {
        match self {
            Profile::Dev => "debug",
            Profile::Release => "release",
        }
    }
}

/// A concrete thing to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Api,
    Identity,
    Forge,
    Web(App),
}

/// Why a request cannot be planned: a malformed request, or a target without an implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Usage(String),
    Unsupported { subject: String, reason: String },
}

pub fn app_of(target: BuildTarget) -> Option<App> {
    match target {
        BuildTarget::CantosStudio => Some(App::Studio),
        BuildTarget::CantosTheatre => Some(App::Theatre),
        BuildTarget::CantosServer
        | BuildTarget::CantosIdentity
        | BuildTarget::CantosForge
        | BuildTarget::All => None,
    }
}

/// Studio is web-only. Theatre production mobile remains UNSUPPORTED; its isolated mock
/// presentation build belongs to `theatre-preview` and cannot establish native listening.
pub fn app_surface(app: App, surface: Option<Surface>) -> Result<(), Refusal> {
    match (app, surface) {
        (_, Some(Surface::Web)) => Ok(()),
        (App::Studio, Some(Surface::Mobile)) => Err(Refusal::Usage(
            "Cantos Studio is web-only; --mobile is out of scope".into(),
        )),
        (App::Studio, None) => Err(Refusal::Usage(
            "cantos-studio is web-only: pass --web".into(),
        )),
        (App::Theatre, Some(Surface::Mobile)) => Err(Refusal::Unsupported {
            subject: "cantos-theatre --mobile".into(),
            reason: format!(
                "{THEATRE_MOBILE_DIR} contains an isolated mock UI; use theatre-preview for it. Production native listening remains unavailable"
            ),
        }),
        (App::Theatre, None) => Err(Refusal::Usage(
            "cantos-theatre has two surfaces: pass --web (implemented) or --mobile (UNSUPPORTED: production native listening; use theatre-preview for the Desktop Preview)".into(),
        )),
    }
}

pub fn resolve(target: BuildTarget, surface: Option<Surface>) -> Result<Vec<Unit>, Refusal> {
    match (target, app_of(target)) {
        (_, Some(app)) => app_surface(app, surface).map(|()| vec![Unit::Web(app)]),
        (BuildTarget::CantosServer, None) if surface.is_none() => Ok(vec![Unit::Api]),
        (BuildTarget::CantosIdentity, None) if surface.is_none() => Ok(vec![Unit::Identity]),
        (BuildTarget::CantosForge, None) if surface.is_none() => Ok(vec![Unit::Forge]),
        (BuildTarget::All, None) if surface.is_none() => Ok(vec![
            Unit::Api,
            Unit::Identity,
            Unit::Forge,
            Unit::Web(App::Studio),
            Unit::Web(App::Theatre),
        ]),
        _ => Err(Refusal::Usage(
            "--web/--mobile select an app surface; cantos-server, cantos-identity, cantos-forge and all take no surface flag".into(),
        )),
    }
}

/// `trunk <subcommand> --locked [--release]` in the app's Web directory.
///
/// `NO_COLOR=true` matches the documented commands: Trunk 0.21 parses `NO_COLOR` as a boolean
/// and rejects an inherited `NO_COLOR=1`.
pub fn trunk(workspace: &Workspace, app: App, subcommand: &str, profile: Profile) -> CommandSpec {
    CommandSpec::new("trunk", workspace.path(app.web_dir()))
        .env("NO_COLOR", "true")
        .args([subcommand, "--locked"])
        .arg_if(profile == Profile::Release, "--release")
}

/// Trunk's environment spellings of `--features`, `--all-features` and `--no-default-features`.
const TRUNK_FEATURE_ENV: [&str; 3] = [
    "TRUNK_BUILD_FEATURES",
    "TRUNK_BUILD_ALL_FEATURES",
    "TRUNK_BUILD_NO_DEFAULT_FEATURES",
];

/// The locked release Trunk build of one Web app into a caller-owned, fresh `dist`: the app's
/// own `Trunk.toml`, its explicit `index.html` entrypoint and the root public URL, so neither an
/// inherited default nor an older output directory can decide which bytes were built. Used by
/// every command that inspects or ships the release bundle (`web-performance`, `deploy`).
pub fn release_bundle(workspace: &Workspace, app: App, dist: &Path) -> CommandSpec {
    // Trunk reads its Cargo feature selection from these variables when no flag is given. A
    // shipped bundle never takes its features from the ambient environment: Theatre's
    // development previews (`--features preview`) must not reach Hosting by accident.
    TRUNK_FEATURE_ENV
        .into_iter()
        .fold(
            trunk(workspace, app, "build", Profile::Release),
            CommandSpec::unset,
        )
        .args([
            "--config".to_owned(),
            workspace
                .path(app.web_dir())
                .join("Trunk.toml")
                .display()
                .to_string(),
            "--dist".to_owned(),
            dist.display().to_string(),
            "--public-url".to_owned(),
            "/".to_owned(),
            "index.html".to_owned(),
        ])
}

pub fn api_binaries(workspace: &Workspace, profile: Profile, bin: Option<&str>) -> CommandSpec {
    let command =
        CommandSpec::new("cargo", workspace.root()).args(["build", "--locked", "-p", API_PACKAGE]);
    let command = match bin {
        Some(bin) => command.args(["--bin", bin]),
        None => command.args(["--bins"]),
    };
    command.arg_if(profile == Profile::Release, "--release")
}

/// `cargo build --locked -p cantos-identity --features service --bin cantos-identity`. The
/// executable needs the `service` feature; the default pure core has no binary and no database.
pub fn identity_binary(workspace: &Workspace, profile: Profile) -> CommandSpec {
    CommandSpec::new("cargo", workspace.root())
        .args([
            "build",
            "--locked",
            "-p",
            IDENTITY_PACKAGE,
            "--features",
            IDENTITY_SERVICE_FEATURE,
            "--bin",
            IDENTITY_BIN,
        ])
        .arg_if(profile == Profile::Release, "--release")
}

/// `cargo build --locked -p cantos-forge --bins [--release]`. Building never runs Forge.
pub fn forge_binary(workspace: &Workspace, profile: Profile) -> CommandSpec {
    CommandSpec::new("cargo", workspace.root())
        .args(["build", "--locked", "-p", FORGE_PACKAGE, "--bins"])
        .arg_if(profile == Profile::Release, "--release")
}

pub fn step(workspace: &Workspace, unit: Unit, profile: Profile) -> Step {
    match unit {
        Unit::Api => Step::new(
            format!("build {API_PACKAGE} ({})", profile.label()),
            &[Tool::Cargo],
            vec![api_binaries(workspace, profile, None)],
        ),
        Unit::Identity => Step::new(
            format!("build {IDENTITY_PACKAGE} ({})", profile.label()),
            &[Tool::Cargo],
            vec![identity_binary(workspace, profile)],
        ),
        Unit::Forge => Step::new(
            format!("build {FORGE_PACKAGE} ({})", profile.label()),
            &[Tool::Cargo],
            vec![forge_binary(workspace, profile)],
        ),
        Unit::Web(app) => Step::new(
            format!("build {} --web ({})", app.cli_name(), profile.label()),
            &[Tool::Cargo, Tool::Wasm32Target, Tool::Trunk],
            vec![trunk(workspace, app, "build", profile)],
        ),
    }
}

pub fn plan(
    workspace: &Workspace,
    target: BuildTarget,
    surface: Option<Surface>,
    profile: Profile,
) -> Result<Vec<Step>, Refusal> {
    Ok(resolve(target, surface)?
        .into_iter()
        .map(|unit| step(workspace, unit, profile))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(target: BuildTarget, surface: Option<Surface>, profile: Profile) -> Vec<String> {
        let workspace = Workspace::discover().unwrap();
        plan(&workspace, target, surface, profile)
            .unwrap()
            .iter()
            .flat_map(|step| step.commands.iter())
            .map(|command| command.display(workspace.root()))
            .collect()
    }

    #[test]
    fn api_builds_every_binary_with_cargo() {
        assert_eq!(
            lines(BuildTarget::CantosServer, None, Profile::Dev),
            ["cargo build --locked -p cantos-server --bins"]
        );
        assert_eq!(
            lines(BuildTarget::CantosServer, None, Profile::Release),
            ["cargo build --locked -p cantos-server --bins --release"]
        );
    }

    #[test]
    fn identity_builds_its_executable_with_the_service_feature_and_takes_no_surface() {
        assert_eq!(
            lines(BuildTarget::CantosIdentity, None, Profile::Dev),
            ["cargo build --locked -p cantos-identity --features service --bin cantos-identity"]
        );
        assert_eq!(
            lines(BuildTarget::CantosIdentity, None, Profile::Release),
            ["cargo build --locked -p cantos-identity --features service --bin cantos-identity --release"]
        );
        for surface in [Surface::Web, Surface::Mobile] {
            assert!(matches!(
                resolve(BuildTarget::CantosIdentity, Some(surface)),
                Err(Refusal::Usage(message)) if message.contains("cantos-identity")
            ));
        }
    }

    #[test]
    fn forge_builds_its_cli_with_cargo_and_takes_no_surface() {
        assert_eq!(
            lines(BuildTarget::CantosForge, None, Profile::Dev),
            ["cargo build --locked -p cantos-forge --bins"]
        );
        assert_eq!(
            lines(BuildTarget::CantosForge, None, Profile::Release),
            ["cargo build --locked -p cantos-forge --bins --release"]
        );
        for surface in [Surface::Web, Surface::Mobile] {
            assert!(matches!(
                resolve(BuildTarget::CantosForge, Some(surface)),
                Err(Refusal::Usage(message)) if message.contains("cantos-forge")
            ));
        }
    }

    #[test]
    fn web_targets_build_with_trunk_in_their_own_directory() {
        assert_eq!(
            lines(
                BuildTarget::CantosStudio,
                Some(Surface::Web),
                Profile::Release
            ),
            ["(in apps/cantos-studio/web) NO_COLOR=true trunk build --locked --release"]
        );
        assert_eq!(
            lines(BuildTarget::CantosTheatre, Some(Surface::Web), Profile::Dev),
            ["(in apps/cantos-theatre/web) NO_COLOR=true trunk build --locked"]
        );
    }

    #[test]
    fn the_release_bundle_build_pins_config_dist_public_url_and_entrypoint() {
        let workspace = Workspace::discover().unwrap();
        let dist = std::path::PathBuf::from("/tmp/run with space/dist");
        for app in App::ALL {
            let command = release_bundle(&workspace, app, &dist);
            let ordinary = step(&workspace, Unit::Web(app), Profile::Release);
            assert!(command.args.starts_with(&ordinary.commands[0].args));
            assert_eq!(command.cwd, ordinary.commands[0].cwd);
            assert_eq!(command.env, ordinary.commands[0].env);
            // No ambient Cargo feature selection can reach a shipped bundle (previews are a
            // Cargo feature); an ordinary local build keeps honoring it for the explicit lane.
            for variable in [
                "TRUNK_BUILD_FEATURES",
                "TRUNK_BUILD_ALL_FEATURES",
                "TRUNK_BUILD_NO_DEFAULT_FEATURES",
            ] {
                assert!(
                    command.env_remove.iter().any(|name| name == variable),
                    "{variable}"
                );
                assert!(ordinary.commands[0].env_remove.is_empty());
            }
            let config = workspace
                .path(app.web_dir())
                .join("Trunk.toml")
                .display()
                .to_string();
            assert_eq!(
                command.args[command.args.len() - 7..],
                [
                    "--config",
                    config.as_str(),
                    "--dist",
                    "/tmp/run with space/dist",
                    "--public-url",
                    "/",
                    "index.html"
                ]
            );
        }
    }

    #[test]
    fn all_builds_server_identity_forge_and_both_web_clients_but_no_mobile() {
        assert_eq!(
            resolve(BuildTarget::All, None).unwrap(),
            [
                Unit::Api,
                Unit::Identity,
                Unit::Forge,
                Unit::Web(App::Studio),
                Unit::Web(App::Theatre)
            ]
        );
    }

    #[test]
    fn studio_mobile_is_out_of_scope_and_theatre_mobile_remains_reserved() {
        assert_eq!(
            resolve(BuildTarget::CantosStudio, Some(Surface::Mobile)),
            Err(Refusal::Usage(
                "Cantos Studio is web-only; --mobile is out of scope".into()
            ))
        );
        assert_eq!(
            resolve(BuildTarget::CantosStudio, None),
            Err(Refusal::Usage(
                "cantos-studio is web-only: pass --web".into()
            ))
        );
        assert!(matches!(
            resolve(BuildTarget::CantosTheatre, Some(Surface::Mobile)),
            Err(Refusal::Unsupported { subject, .. }) if subject == "cantos-theatre --mobile"
        ));
        assert!(matches!(
            resolve(BuildTarget::CantosTheatre, None),
            Err(Refusal::Usage(message)) if message.contains("--web")
        ));
        assert!(matches!(
            resolve(BuildTarget::CantosServer, Some(Surface::Web)),
            Err(Refusal::Usage(_))
        ));
        assert!(matches!(
            resolve(BuildTarget::All, Some(Surface::Mobile)),
            Err(Refusal::Usage(_))
        ));
    }
}
