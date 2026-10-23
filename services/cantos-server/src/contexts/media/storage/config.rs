//! Checked, non-secret topology. Secret values are injected only at composition time.
use super::super::domain::object_storage::{StorageError, StorageTargetId};
use super::s3::S3Settings;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const MAX_CONFIG_BYTES: u64 = 65_536;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    storage: Topology,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Topology {
    active_write_target: String,
    targets: BTreeMap<String, RawTarget>,
}
#[derive(Deserialize)]
#[serde(tag = "driver", rename_all = "snake_case", deny_unknown_fields)]
enum RawTarget {
    Filesystem {
        private_root: PathBuf,
    },
    S3 {
        endpoint: String,
        region: String,
        private_bucket: String,
        force_path_style: bool,
        access_key_env: String,
        secret_key_env: String,
        session_token_env: Option<String>,
        connect_timeout_ms: u64,
        request_timeout_ms: u64,
        conditional_put: bool,
        range_reads: bool,
        presigned_reads: bool,
    },
}

pub(super) struct StorageConfig {
    pub active: StorageTargetId,
    pub targets: BTreeMap<StorageTargetId, TargetConfig>,
}
pub(super) enum TargetConfig {
    Filesystem(PathBuf),
    S3 {
        settings: S3Settings,
        access_key_env: String,
        secret_key_env: String,
        session_token_env: Option<String>,
    },
}

impl StorageConfig {
    pub fn load(path: &Path) -> Result<Self, StorageError> {
        if !path.is_absolute() {
            return Err(StorageError::InvalidTarget);
        }
        let file = std::fs::File::open(path).map_err(|_| StorageError::Unavailable)?;
        let metadata = file.metadata().map_err(|_| StorageError::Unavailable)?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_CONFIG_BYTES {
            return Err(StorageError::InvalidTarget);
        }
        use std::io::Read;
        let mut contents = String::new();
        file.take(MAX_CONFIG_BYTES + 1)
            .read_to_string(&mut contents)
            .map_err(|_| StorageError::Unavailable)?;
        if contents.len() as u64 > MAX_CONFIG_BYTES {
            return Err(StorageError::InvalidTarget);
        }
        Self::parse(&contents)
    }
    pub(super) fn parse(contents: &str) -> Result<Self, StorageError> {
        let raw: Document = toml::from_str(contents).map_err(|_| StorageError::InvalidTarget)?;
        if raw.version != 1 || raw.storage.targets.is_empty() || raw.storage.targets.len() > 16 {
            return Err(StorageError::InvalidTarget);
        }
        let active = StorageTargetId::parse(&raw.storage.active_write_target)?;
        let mut targets = BTreeMap::new();
        for (name, raw) in raw.storage.targets {
            let id = StorageTargetId::parse(&name)?;
            let target = match raw {
                RawTarget::Filesystem { private_root } => {
                    if !private_root.is_absolute()
                        || private_root.components().any(|c| {
                            matches!(
                                c,
                                std::path::Component::ParentDir | std::path::Component::CurDir
                            )
                        })
                    {
                        return Err(StorageError::InvalidTarget);
                    }
                    TargetConfig::Filesystem(private_root)
                }
                RawTarget::S3 {
                    endpoint,
                    region,
                    private_bucket,
                    force_path_style,
                    access_key_env,
                    secret_key_env,
                    session_token_env,
                    connect_timeout_ms,
                    request_timeout_ms,
                    conditional_put,
                    range_reads,
                    presigned_reads,
                } => {
                    for name in [&access_key_env, &secret_key_env]
                        .into_iter()
                        .chain(session_token_env.as_ref())
                    {
                        if !valid_secret_name(name) {
                            return Err(StorageError::InvalidTarget);
                        }
                    }
                    if access_key_env == secret_key_env
                        || session_token_env
                            .as_ref()
                            .is_some_and(|n| n == &access_key_env || n == &secret_key_env)
                    {
                        return Err(StorageError::InvalidTarget);
                    }
                    if id == active && !conditional_put {
                        return Err(StorageError::UnsupportedCapability(super::super::domain::object_storage::StorageCapability::ConditionalCreate));
                    }
                    let settings = S3Settings {
                        endpoint,
                        region,
                        bucket: private_bucket,
                        force_path_style,
                        connect_timeout_ms,
                        request_timeout_ms,
                        conditional_put,
                        range_reads,
                        presigned_reads,
                    };
                    settings.validate()?;
                    TargetConfig::S3 {
                        settings,
                        access_key_env,
                        secret_key_env,
                        session_token_env,
                    }
                }
            };
            targets.insert(id, target);
        }
        if !targets.contains_key(&active) {
            return Err(StorageError::InvalidTarget);
        }
        Ok(Self { active, targets })
    }
}

fn valid_secret_name(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && name.as_bytes()[0].is_ascii_uppercase()
        && name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    const LOCAL: &str = "version=1\n[storage]\nactive_write_target='legacy-filesystem'\n[storage.targets.legacy-filesystem]\ndriver='filesystem'\nprivate_root='/private/tmp/cantos-private'\n";
    #[test]
    fn topology_rejects_unknown_inconsistent_or_noncanonical_configuration() {
        assert!(StorageConfig::parse(LOCAL).is_ok());
        for text in [
            LOCAL.replace("version=1", "version=2"),
            LOCAL.replace("version=1", "version=1\nunknown=true"),
            LOCAL.replace("private_root=", "typo="),
            LOCAL.replace("'/private/tmp/cantos-private'", "'relative'"),
            LOCAL.replace(
                "active_write_target='legacy-filesystem'",
                "active_write_target='missing'",
            ),
            format!("{LOCAL}secret='never-accepted'\n"),
        ] {
            assert!(matches!(
                StorageConfig::parse(&text),
                Err(StorageError::InvalidTarget)
            ));
        }
    }
    #[test]
    fn vps_default_profile_selects_only_the_persistent_filesystem_target() {
        let text = include_str!("../../../../../../infra/vps/config/storage.filesystem.toml");
        let config = StorageConfig::parse(text).unwrap();
        let id = StorageTargetId::parse("legacy-filesystem").unwrap();
        assert_eq!(config.active, id);
        assert_eq!(config.targets.len(), 1);
        assert!(matches!(
            config.targets.get(&id),
            Some(TargetConfig::Filesystem(root)) if root == Path::new("/var/lib/cantos/private")
        ));
    }

    #[test]
    fn long_van_profile_parses_as_non_secret_topology_with_conservative_capabilities() {
        let text = include_str!("../../../../config/storage.longvan.example.toml");
        let config = StorageConfig::parse(text).unwrap();
        let id = StorageTargetId::parse("longvan-hn1").unwrap();
        assert_eq!(config.active, id);
        let Some(TargetConfig::S3 {
            settings,
            access_key_env,
            secret_key_env,
            session_token_env,
        }) = config.targets.get(&id)
        else {
            panic!("the Long Vân profile must be an S3 target");
        };
        assert!(settings.endpoint.starts_with("https://"));
        assert!(settings.force_path_style && settings.conditional_put && settings.range_reads);
        // Direct grants stay off until live version-pin, scoping and expiry acceptance passes.
        assert!(!settings.presigned_reads);
        assert_eq!(access_key_env, "CANTOS_S3_ACCESS_KEY_ID");
        assert_eq!(secret_key_env, "CANTOS_S3_SECRET_ACCESS_KEY");
        assert!(session_token_env.is_none());
    }

    #[test]
    fn credential_source_names_are_checked_without_reading_environment() {
        for name in ["CANTOS_ACCESS_KEY", "A1"] {
            assert!(valid_secret_name(name));
        }
        for name in ["", "secret", "KEY=bad", "../secret", "A\nB", "1KEY"] {
            assert!(!valid_secret_name(name));
        }
    }
}
