//! A bounded process inventory, not a deployment language. Deserialization is followed by
//! semantic validation. The inventory describes runnable native processes and independently
//! managed external infrastructure; no container-managed development service remains.
use std::{collections::BTreeMap, path::Path};

use anyhow::{ensure, Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    Native,
    External,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Implementation {
    Implemented,
    Configured,
    Planned,
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
            ("cantos-identity", Runtime::Native),
            ("cantos-server", Runtime::Native),
            ("object-storage", Runtime::External),
        ];
        ensure!(
            manifest.services.len() == expected.len(),
            "inventory must contain the three reviewed services"
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
                    .all(|key| key.starts_with("CANTOS_")),
                "{name}: undeclared environment namespace (a database URL is not a Cantos input)"
            );
        }
        for name in manifest.services.keys() {
            manifest.visit(name, &mut Vec::new())?;
        }
        // Each native process owns exactly one embedded file; neither needs another process to
        // start. The Server reaches Identity only through its explicit private socket.
        let server = manifest.service("cantos-server")?;
        ensure!(
            server.required_dependencies.is_empty()
                && server
                    .optional_dependencies
                    .iter()
                    .map(String::as_str)
                    .eq(["cantos-identity", "object-storage"]),
            "cantos-server has no required dependency; Identity and object storage are optional"
        );
        let identity = manifest.service("cantos-identity")?;
        ensure!(
            identity.required_dependencies.is_empty()
                && identity.optional_dependencies.is_empty()
                && identity.ports.is_empty(),
            "cantos-identity depends on nothing and listens on no TCP port"
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
