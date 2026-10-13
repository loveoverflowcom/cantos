//! Resolve only reviewed local runtime inputs. Secrets are paths, never command-line values.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::manifest::{read_small, Container, Kind};

pub const MANAGER: &str = "io.cantos.manager";
pub const PROJECT: &str = "io.cantos.project";
pub const INSTANCE: &str = "io.cantos.instance";
pub const SERVICE: &str = "io.cantos.service";
pub const SIGNATURE: &str = "io.cantos.configuration";
pub const IMAGE: &str = "io.cantos.image";

pub fn instance_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

pub fn labels(instance: &str, service: &str) -> BTreeMap<String, String> {
    [
        (MANAGER, "xtask-services-v1"),
        (PROJECT, "cantos"),
        (INSTANCE, instance),
        (SERVICE, service),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
}

pub fn owned(actual: &BTreeMap<String, String>, instance: &str, service: &str) -> bool {
    labels(instance, service)
        .iter()
        .all(|(key, value)| actual.get(key) == Some(value))
}

#[derive(Debug)]
pub struct Plan {
    pub volume: String,
    pub service: String,
    pub instance: String,
    pub signature: String,
    pub image: String,
    pub create: Vec<String>,
    pub configtest: Option<Vec<String>>,
}

impl Plan {
    pub fn resolve(
        root: &Path,
        instance: &str,
        service: &str,
        container: &Container,
        env: &BTreeMap<String, String>,
    ) -> Result<Self> {
        ensure!(
            instance_valid(instance),
            "instance must be 1–32 lowercase letters/digits/hyphens, starting with a letter/digit"
        );
        let port = env
            .get(&container.port_env)
            .map(|s| s.parse::<u16>())
            .transpose()
            .context("port must be an integer in 1024..65535")?
            .unwrap_or(container.default_port);
        ensure!(port >= 1024, "port must be in 1024..65535");
        let name = format!("cantos-{instance}-{service}");
        let volume = format!("{name}-data");
        let mut args = vec![
            "create".into(),
            format!("--name={name}"),
            "--pull=missing".into(),
            "--restart=no".into(),
            "--http-proxy=false".into(),
        ];
        append_labels(&mut args, &labels(instance, service));
        args.extend([
            format!("--publish=127.0.0.1:{port}:{}", container.container_port),
            format!("--volume={volume}:{}:nocreate", container.data_path),
            format!(
                "--health-cmd={}",
                serde_json::to_string(&container.health_command)?
            ),
            "--health-interval=10s".into(),
            "--health-timeout=5s".into(),
            "--health-retries=3".into(),
        ]);
        let mut config_bytes = String::new();
        let configtest = match container.kind {
            Kind::Postgresql => {
                let file = private_input(root, env, "CANTOS_POSTGRES_PASSWORD_FILE", false)?;
                ensure!(
                    std::fs::metadata(&file)?.len() > 0,
                    "password file must be nonempty"
                );
                args.extend([
                    format!(
                        "--volume={}:{}:ro",
                        file.display(),
                        "/run/secrets/postgres-password"
                    ),
                    "--env=POSTGRES_PASSWORD_FILE=/run/secrets/postgres-password".into(),
                    "--env=POSTGRES_HOST_AUTH_METHOD=scram-sha-256".into(),
                    "--env=POSTGRES_INITDB_ARGS=--auth-host=scram-sha-256".into(),
                    container.image.clone(),
                ]);
                None
            }
            Kind::Kanidm => {
                let dir = private_input(root, env, "CANTOS_KANIDM_CONFIG_DIR", true)?;
                for file in ["server.toml", "chain.pem", "key.pem"] {
                    private_path(root, &dir.join(file), false)?;
                }
                config_bytes = read_small(&dir.join("server.toml"))?;
                validate_kanidm(&config_bytes, Some(port))?;
                let mount = format!("--volume={}:{}:ro", dir.display(), "/config");
                args.extend([
                    mount.clone(),
                    "--entrypoint=/sbin/kanidmd".into(),
                    container.image.clone(),
                    "server".into(),
                    "-c".into(),
                    "/config/server.toml".into(),
                ]);
                let mut check = vec![
                    "run".into(),
                    "--rm".into(),
                    "--network=none".into(),
                    "--pull=missing".into(),
                    "--http-proxy=false".into(),
                    "--tmpfs=/data".into(),
                    mount,
                    "--entrypoint=/sbin/kanidmd".into(),
                ];
                append_labels(&mut check, &labels(instance, service));
                check.extend([
                    container.image.clone(),
                    "configtest".into(),
                    "-c".into(),
                    "/config/server.toml".into(),
                ]);
                Some(check)
            }
        };
        let mut hash = Sha256::new();
        hash.update(serde_json::to_vec(&args)?);
        hash.update(config_bytes.as_bytes());
        let signature = format!("{:x}", hash.finalize());
        args.insert(1, format!("--label={SIGNATURE}={signature}"));
        Ok(Self {
            volume,
            service: service.into(),
            instance: instance.into(),
            signature,
            image: container.image.clone(),
            create: args,
            configtest,
        })
    }
}

pub fn append_labels(args: &mut Vec<String>, labels: &BTreeMap<String, String>) {
    args.extend(
        labels
            .iter()
            .map(|(key, value)| format!("--label={key}={value}")),
    );
}

fn private_input(
    root: &Path,
    env: &BTreeMap<String, String>,
    key: &str,
    directory: bool,
) -> Result<PathBuf> {
    let value = env
        .get(key)
        .with_context(|| format!("export {key} explicitly; see the service env.example"))?;
    private_path(root, Path::new(value), directory)
}

fn private_path(root: &Path, path: &Path, directory: bool) -> Result<PathBuf> {
    ensure!(
        path.is_absolute(),
        "private runtime input must be an absolute path"
    );
    let metadata =
        std::fs::symlink_metadata(path).context("private runtime input is unavailable")?;
    ensure!(
        if directory {
            metadata.is_dir()
        } else {
            metadata.is_file()
        },
        "private input must be a regular {} (no symlinks)",
        if directory { "directory" } else { "file" }
    );
    let canonical = path.canonicalize()?;
    ensure!(
        !canonical.starts_with(root.canonicalize()?),
        "private runtime inputs must stay outside the repository"
    );
    ensure!(
        !canonical
            .ancestors()
            .any(|parent| parent.join(".git").exists()),
        "private input must stay outside any Git checkout"
    );
    let value = canonical.to_str().context("runtime path must be UTF-8")?;
    ensure!(
        !value.contains([':', ',', '\n', '\r']),
        "runtime path contains a reserved mount delimiter"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.uid() == unsafe { libc::getuid() } && metadata.mode() & 0o077 == 0,
            "private input must be caller-owned with no group/other permissions"
        );
    }
    Ok(canonical)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KanidmConfig {
    version: String,
    bindaddress: String,
    db_path: String,
    tls_chain: String,
    tls_key: String,
    domain: String,
    origin: String,
}

pub fn validate_kanidm(text: &str, host_port: Option<u16>) -> Result<()> {
    let config: KanidmConfig = toml::from_str(text)
        .map_err(|_| anyhow::anyhow!("invalid narrow Kanidm evaluation configuration"))?;
    ensure!(
        config.version == "2"
            && config.bindaddress == "[::]:8443"
            && config.db_path == "/data/kanidm.db"
            && config.tls_chain == "/config/chain.pem"
            && config.tls_key == "/config/key.pem",
        "Kanidm evaluation requires fixed version 2, internal bind/data and read-only TLS paths"
    );
    ensure!(
        config.domain.contains('.')
            && config.domain.len() <= 253
            && config.domain.split('.').all(|part| !part.is_empty()
                && part.len() <= 63
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                && !part.starts_with('-')
                && !part.ends_with('-')),
        "invalid Kanidm DNS domain"
    );
    let expected = format!("https://{}:", config.domain);
    let port: u16 = config
        .origin
        .strip_prefix(&expected)
        .context("Kanidm origin must match its HTTPS domain and explicit host port")?
        .parse()
        .context("invalid Kanidm origin port")?;
    ensure!(
        port >= 1024 && host_port.is_none_or(|host| host == port),
        "Kanidm origin must match the configured unprivileged host port"
    );
    Ok(())
}
