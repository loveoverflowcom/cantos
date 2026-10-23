//! The process inventory (`services list|validate`). It describes the native Server and
//! Identity processes and the external object store; it starts nothing. The Podman lifecycle
//! (`up|down|logs|status`) was retired with PostgreSQL and Kanidm, the only services it managed.
mod manifest;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{
    report::{Report, Status},
    retired::Retired,
    workspace::Workspace,
};
use manifest::Manifest;

#[derive(Debug, Args)]
pub struct ServicesArgs {
    /// Retired with the container lifecycle; accepted only so the retired actions can refuse.
    #[arg(long, hide = true)]
    pub instance: Option<String>,
    #[command(subcommand)]
    pub action: Action,
}

#[derive(Debug, Subcommand)]
pub enum Action {
    /// List the typed inventory without probing or starting anything.
    List,
    /// Validate the inventory without running any process.
    Validate,
    /// Retired: no container-managed service remains.
    #[command(hide = true)]
    Up(Retired),
    /// Retired: no container-managed service remains.
    #[command(hide = true)]
    Down(Retired),
    /// Retired: no container-managed service remains.
    #[command(hide = true)]
    Logs(Retired),
    /// Retired: nothing is observed.
    #[command(hide = true)]
    Status(Retired),
}

impl ServicesArgs {
    pub fn is_retired(&self) -> bool {
        matches!(
            self.action,
            Action::Up(_) | Action::Down(_) | Action::Logs(_) | Action::Status(_)
        )
    }
}

pub fn validate(root: &std::path::Path) -> Result<Manifest> {
    Manifest::load(root)
}

pub fn run(workspace: &Workspace, args: &ServicesArgs) -> Result<ExitCode> {
    let manifest = validate(workspace.root())?;
    match &args.action {
        Action::Validate => {
            let mut report = Report::new("services validate");
            report.record(
                "manifest",
                Status::Pass,
                "typed/static validation only; no process was started or observed",
            );
            report.print();
            Ok(report.exit_code())
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
            Ok(ExitCode::SUCCESS)
        }
        Action::Up(_) | Action::Down(_) | Action::Logs(_) | Action::Status(_) => {
            unreachable!("retired actions are refused before dispatch")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;

    fn manifest() -> Manifest {
        Manifest::load(Workspace::discover().unwrap().root()).unwrap()
    }

    #[test]
    fn the_checked_in_inventory_is_the_native_composition_plus_external_storage() {
        let manifest = manifest();
        let names: Vec<_> = manifest.services.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            ["cantos-identity", "cantos-server", "object-storage"]
        );
        let server = manifest.service("cantos-server").unwrap();
        assert!(server.required_dependencies.is_empty());
        assert_eq!(server.ports, [8080]);
        assert!(manifest
            .service("cantos-identity")
            .unwrap()
            .ports
            .is_empty());
    }

    #[test]
    fn no_inventory_text_names_a_retired_database_or_provider() {
        let text = std::fs::read_to_string(
            Workspace::discover()
                .unwrap()
                .path("services/manifest.toml"),
        )
        .unwrap()
        .to_ascii_lowercase();
        for retired in ["postgres", "kanidm", "database_url", "podman_machine"] {
            assert!(!text.contains(retired), "{retired}");
        }
    }

    fn edited(replace: &str, with: &str) -> String {
        let text = std::fs::read_to_string(
            Workspace::discover()
                .unwrap()
                .path("services/manifest.toml"),
        )
        .unwrap();
        assert!(text.contains(replace), "fixture drifted: {replace}");
        text.replacen(replace, with, 1)
    }

    #[test]
    fn a_container_table_a_database_url_or_a_required_identity_is_refused() {
        let with_container = format!(
            "{}\n[services.cantos-server.container]\nkind = \"postgresql\"\nimage = \"x\"\n",
            edited("version = 1", "version = 1")
        );
        assert!(Manifest::parse(&with_container).is_err());
        assert!(Manifest::parse(&edited(
            "\"CANTOS_ENV\"",
            "\"DATABASE_URL\", \"CANTOS_ENV\""
        ))
        .is_err());
        assert!(Manifest::parse(&edited(
            "required_dependencies = []\noptional_dependencies = [\"cantos-identity\", \"object-storage\"]",
            "required_dependencies = [\"cantos-identity\"]\noptional_dependencies = [\"object-storage\"]"
        ))
        .is_err());
        assert!(Manifest::parse(&edited("runtime = \"native\"", "runtime = \"podman\"")).is_err());
    }

    #[test]
    fn identity_owns_no_tcp_port_and_depends_on_nothing() {
        assert!(Manifest::parse(&edited(
            "ports = []\nhealth = \"Native run connects",
            "ports = [9000]\nhealth = \"Native run connects"
        ))
        .is_err());
    }

    #[test]
    fn identity_environment_names_exist_in_the_identity_sources() {
        let workspace = Workspace::discover().unwrap();
        let config =
            std::fs::read_to_string(workspace.path("services/cantos-identity/src/config.rs"))
                .unwrap();
        let mail = std::fs::read_to_string(workspace.path("services/cantos-identity/src/mail.rs"))
            .unwrap();
        for name in &manifest().service("cantos-identity").unwrap().environment {
            let smtp = name.starts_with("CANTOS_IDENTITY_SMTP_");
            let suffix = name.trim_start_matches("CANTOS_IDENTITY_SMTP_");
            assert!(
                config.contains(&format!("\"{name}\""))
                    || (smtp && mail.contains(&format!("\"{name}\"")))
                    || (smtp && mail.contains(&format!("\"{suffix}\""))),
                "{name} is not read by the Identity executable"
            );
        }
    }
}
