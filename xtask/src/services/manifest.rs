//! A bounded process inventory, not a deployment language. Deserialization is followed by
//! semantic validation before any value can reach lifecycle effects.
use std::{collections::BTreeMap, path::Path};

use anyhow::{ensure, Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    Native,
    Podman,
    External,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Implementation {
    Implemented,
    Configured,
    Planned,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Postgresql,
    Kanidm,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Container {
    pub kind: Kind,
    pub image: String,
    pub data_path: String,
    pub port_env: String,
    pub container_port: u16,
    pub default_port: u16,
    pub health_command: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub runtime: Runtime,
    pub implementation: Implementation,
    pub owner: String,
    pub required_dependencies: Vec<String>,
    pub optional_dependencies: Vec<String>,
    pub development: String,
    pub production: String,
    pub environment: Vec<String>,
    pub ports: Vec<u16>,
    pub health: String,
    pub persistence: String,
    pub lifecycle: String,
    pub container: Option<Container>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    version: u32,
    pub services: BTreeMap<String, Service>,
}

impl Manifest {
    pub fn load(root: &Path) -> Result<Self> {
        Self::parse(&read_small(&root.join("services/manifest.toml"))?)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let manifest: Self = toml::from_str(text).context("invalid services/manifest.toml")?;
        ensure!(
            manifest.version == 1,
            "unsupported service manifest version"
        );
        let expected = [
            ("cantos-server", Runtime::Native),
            ("kanidm", Runtime::Podman),
            ("object-storage", Runtime::External),
            ("postgresql", Runtime::Podman),
        ];
        ensure!(
            manifest.services.len() == expected.len(),
            "inventory must contain the four reviewed services"
        );
        for (name, runtime) in expected {
            let service = manifest.service(name)?;
            ensure!(
                service.runtime == runtime,
                "{name}: runtime differs from the reviewed boundary"
            );
            for (field, value) in [
                ("owner", &service.owner),
                ("development", &service.development),
                ("production", &service.production),
                ("health", &service.health),
                ("persistence", &service.persistence),
                ("lifecycle", &service.lifecycle),
            ] {
                ensure!(!value.trim().is_empty(), "{name}: {field} must be nonempty");
            }
            let mut dependencies = std::collections::BTreeSet::new();
            for dependency in service
                .required_dependencies
                .iter()
                .chain(&service.optional_dependencies)
            {
                ensure!(
                    dependency != name && manifest.services.contains_key(dependency),
                    "{name}: invalid dependency {dependency}"
                );
                ensure!(
                    dependencies.insert(dependency),
                    "{name}: duplicate dependency {dependency}"
                );
            }
            ensure!(
                service.ports.iter().all(|port| *port >= 1024),
                "{name}: invalid port"
            );
            ensure!(
                service
                    .environment
                    .iter()
                    .all(|key| key.starts_with("CANTOS_") || key == "DATABASE_URL"),
                "{name}: undeclared environment namespace"
            );
            match (&service.container, runtime) {
                (None, Runtime::Native | Runtime::External) => {}
                (Some(container), Runtime::Podman) => container.validate(name, service)?,
                _ => anyhow::bail!("{name}: container configuration must match runtime"),
            }
        }
        for name in manifest.services.keys() {
            manifest.visit(name, &mut Vec::new())?;
        }
        ensure!(
            manifest.service("cantos-server")?.required_dependencies == ["postgresql"],
            "cantos-server must require PostgreSQL only"
        );
        ensure!(
            manifest
                .service("postgresql")?
                .required_dependencies
                .is_empty()
                && manifest.service("kanidm")?.required_dependencies.is_empty(),
            "local containers have no required dependencies"
        );
        Ok(manifest)
    }

    fn visit<'a>(&'a self, name: &'a str, stack: &mut Vec<&'a str>) -> Result<()> {
        ensure!(
            !stack.contains(&name),
            "required dependency cycle at {name}"
        );
        stack.push(name);
        for dependency in &self.service(name)?.required_dependencies {
            self.visit(dependency, stack)?;
        }
        stack.pop();
        Ok(())
    }

    pub fn service(&self, name: &str) -> Result<&Service> {
        self.services
            .get(name)
            .with_context(|| format!("unknown service {name}; use services list"))
    }
}

impl Container {
    fn validate(&self, name: &str, service: &Service) -> Result<()> {
        let (expected_name, registry, data, env, port, health): (_, _, _, _, _, &[&str]) =
            match self.kind {
                Kind::Postgresql => (
                    "postgresql",
                    "docker.io/library/postgres:",
                    "/var/lib/postgresql/data",
                    "CANTOS_POSTGRES_PORT",
                    5432,
                    &[
                        "pg_isready",
                        "-h",
                        "127.0.0.1",
                        "-p",
                        "5432",
                        "-U",
                        "postgres",
                        "-d",
                        "postgres",
                    ],
                ),
                Kind::Kanidm => (
                    "kanidm",
                    "docker.io/kanidm/server:",
                    "/data",
                    "CANTOS_KANIDM_PORT",
                    8443,
                    &[
                        "/sbin/kanidmd",
                        "scripting",
                        "healthcheck",
                        "--verify-tls",
                        "-c",
                        "/config/server.toml",
                    ],
                ),
            };
        ensure!(name == expected_name, "{name}: container kind mismatch");
        let tag = self
            .image
            .strip_prefix(registry)
            .context("only the reviewed official registry/image is permitted")?;
        let version = tag.split('-').next().unwrap_or_default();
        let numbers: Vec<_> = version.split('.').collect();
        ensure!(
            numbers.len() == 3 - usize::from(self.kind == Kind::Postgresql)
                && numbers
                    .iter()
                    .all(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit())),
            "{name}: image needs an explicit patch version"
        );
        ensure!(
            tag.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".-".contains(&c)),
            "{name}: invalid image tag"
        );
        if self.kind == Kind::Postgresql {
            ensure!(
                tag.starts_with("17.") && tag == format!("{version}-bookworm"),
                "PostgreSQL requires reviewed major 17/bookworm"
            );
        } else {
            ensure!(
                tag == "1.10.5",
                "Kanidm config/health contract is pinned to 1.10.5"
            );
        }
        ensure!(
            self.data_path == data
                && self.port_env == env
                && self.container_port == port
                && self.default_port >= 1024,
            "{name}: invalid mount or port contract"
        );
        ensure!(
            self.health_command
                .iter()
                .map(String::as_str)
                .eq(health.iter().copied()),
            "{name}: health argv differs from reviewed probe"
        );
        ensure!(
            service.environment.iter().any(|key| key == env) && service.ports == [port],
            "{name}: inventory ports/environment mismatch"
        );
        Ok(())
    }
}

pub fn read_small(path: &Path) -> Result<String> {
    use std::io::Read;
    ensure!(
        std::fs::symlink_metadata(path)?.is_file(),
        "{} must be a regular file (no symlinks)",
        path.display()
    );
    let file =
        std::fs::File::open(path).with_context(|| format!("could not open {}", path.display()))?;
    ensure!(
        file.metadata()?.is_file(),
        "{} must be a regular file",
        path.display()
    );
    let mut text = String::new();
    file.take(64 * 1024 + 1).read_to_string(&mut text)?;
    ensure!(text.len() <= 64 * 1024, "{} exceeds 64 KiB", path.display());
    Ok(text)
}
