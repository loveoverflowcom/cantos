//! Build plans: which Cargo or Trunk invocation produces each target, per profile.
//!
//! `dev` is the default unoptimized local build. `--release` is an optimized local artifact
//! (`cargo build --release`, `trunk build --release`); it is not a production image, a signed
//! bundle or a deployment, none of which exist yet.

use clap::ValueEnum;

use crate::{
    command::CommandSpec,
    config::{App, API_PACKAGE, THEATRE_MOBILE_DIR},
    pipeline::Step,
    tools::Tool,
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BuildTarget {
    /// The Axum/PostgreSQL backend and its CLIs (`services/cantos-api`).
    CantosApi,
    /// Cantos Studio, the creator product (`apps/cantos-studio`).
    CantosStudio,
    /// Cantos Theatre, the listener product (`apps/cantos-theatre`).
    #[value(alias = "cantos")]
    CantosTheatre,
    /// The API plus every implemented Web client.
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
        BuildTarget::CantosApi | BuildTarget::All => None,
    }
}

/// Studio is web-only. Theatre mobile is a reserved extension point, refused as
/// UNSUPPORTED until a Gradle project exists, never simulated.
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
                "{THEATRE_MOBILE_DIR} has no Gradle project; see its README.md for the planned CMP structure"
            ),
        }),
        (App::Theatre, None) => Err(Refusal::Usage(format!(
            "cantos-theatre has two surfaces: pass --web (implemented) or --mobile (UNSUPPORTED: no Gradle project in {THEATRE_MOBILE_DIR})"
        ))),
    }
}

pub fn resolve(target: BuildTarget, surface: Option<Surface>) -> Result<Vec<Unit>, Refusal> {
    match (target, app_of(target)) {
        (_, Some(app)) => app_surface(app, surface).map(|()| vec![Unit::Web(app)]),
        (BuildTarget::CantosApi, None) if surface.is_none() => Ok(vec![Unit::Api]),
        (BuildTarget::All, None) if surface.is_none() => Ok(vec![
            Unit::Api,
            Unit::Web(App::Studio),
            Unit::Web(App::Theatre),
        ]),
        _ => Err(Refusal::Usage(
            "--web/--mobile select an app surface; cantos-api and all take no surface flag".into(),
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

pub fn api_binaries(workspace: &Workspace, profile: Profile, bin: Option<&str>) -> CommandSpec {
    let command =
        CommandSpec::new("cargo", workspace.root()).args(["build", "--locked", "-p", API_PACKAGE]);
    let command = match bin {
        Some(bin) => command.args(["--bin", bin]),
        None => command.args(["--bins"]),
    };
    command.arg_if(profile == Profile::Release, "--release")
}

pub fn step(workspace: &Workspace, unit: Unit, profile: Profile) -> Step {
    match unit {
        Unit::Api => Step::new(
            format!("build {API_PACKAGE} ({})", profile.label()),
            &[Tool::Cargo],
            vec![api_binaries(workspace, profile, None)],
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
            lines(BuildTarget::CantosApi, None, Profile::Dev),
            ["cargo build --locked -p cantos-api --bins"]
        );
        assert_eq!(
            lines(BuildTarget::CantosApi, None, Profile::Release),
            ["cargo build --locked -p cantos-api --bins --release"]
        );
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
    fn all_builds_the_api_and_both_web_clients_but_no_mobile() {
        assert_eq!(
            resolve(BuildTarget::All, None).unwrap(),
            [Unit::Api, Unit::Web(App::Studio), Unit::Web(App::Theatre)]
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
            resolve(BuildTarget::CantosApi, Some(Surface::Web)),
            Err(Refusal::Usage(_))
        ));
        assert!(matches!(
            resolve(BuildTarget::All, Some(Surface::Mobile)),
            Err(Refusal::Usage(_))
        ));
    }
}
