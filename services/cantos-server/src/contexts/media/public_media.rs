//! Explicit origin-delivery host configuration. Nothing is inferred from private previews,
//! browser request headers or a configured private object store. No deployment is performed.
use super::{
    domain::playback_grant::GrantKey, storage::validate_publication_separation,
    FilesystemPublicationStorage, OriginPlayback, PublicMediaOrigin,
};
use std::{path::Path, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicMediaConfigError {
    Incomplete,
    InvalidOrigin,
    InvalidStorage,
    InvalidKey,
}
impl PublicMediaConfigError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Incomplete => "public media requires publication root, HTTPS origin and key file together",
            Self::InvalidOrigin => "invalid public media HTTPS origin",
            Self::InvalidStorage => "invalid or overlapping publication storage root",
            Self::InvalidKey => "playback key must be an owner-only regular file containing a 32-byte key other than the all-zero key",
        }
    }
}

pub struct PublicMediaRuntime {
    pub storage: Arc<FilesystemPublicationStorage>,
    pub playback: OriginPlayback,
}

pub fn from_lookup(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<Option<PublicMediaRuntime>, PublicMediaConfigError> {
    let root = lookup("CANTOS_PUBLICATION_ROOT");
    let origin = lookup("CANTOS_PUBLIC_MEDIA_ORIGIN");
    let key = lookup("CANTOS_PLAYBACK_KEY_FILE");
    if root.is_none() && origin.is_none() && key.is_none() {
        return Ok(None);
    }
    let (Some(root), Some(origin), Some(key)) = (root, origin, key) else {
        return Err(PublicMediaConfigError::Incomplete);
    };
    let origin =
        PublicMediaOrigin::parse(&origin).map_err(|_| PublicMediaConfigError::InvalidOrigin)?;
    let key = read_key(Path::new(&key))?;
    let private_root = lookup("CANTOS_AUDIO_ROOT");
    let private_config = lookup("CANTOS_STORAGE_CONFIG");
    validate_publication_separation(
        Path::new(&root),
        private_root.as_deref().map(Path::new),
        private_config.as_deref().map(Path::new),
    )
    .map_err(|_| PublicMediaConfigError::InvalidStorage)?;
    let storage = Arc::new(
        FilesystemPublicationStorage::new(Path::new(&root))
            .map_err(|_| PublicMediaConfigError::InvalidStorage)?,
    );
    let playback = OriginPlayback::new(origin, key, storage.clone());
    Ok(Some(PublicMediaRuntime { storage, playback }))
}

fn read_key(path: &Path) -> Result<GrantKey, PublicMediaConfigError> {
    #[cfg(unix)]
    {
        use std::{
            io::Read,
            os::unix::fs::{MetadataExt, OpenOptionsExt},
        };
        let invalid = |_| PublicMediaConfigError::InvalidKey;
        if !path.is_absolute() || std::fs::canonicalize(path).map_err(invalid)? != path {
            return Err(PublicMediaConfigError::InvalidKey);
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(invalid)?;
        let metadata = file.metadata().map_err(invalid)?;
        // SAFETY: geteuid has no arguments or memory access preconditions.
        let owner = unsafe { libc::geteuid() };
        if !metadata.is_file()
            || metadata.len() != 32
            || metadata.uid() != owner
            || !matches!(metadata.mode() & 0o7777, 0o400 | 0o600)
        {
            return Err(PublicMediaConfigError::InvalidKey);
        }
        let mut bytes = Vec::new();
        file.take(33).read_to_end(&mut bytes).map_err(invalid)?;
        GrantKey::new(&bytes).map_err(|_| PublicMediaConfigError::InvalidKey)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(PublicMediaConfigError::InvalidKey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_configuration_stays_unavailable_and_partial_configuration_refuses_startup() {
        assert!(from_lookup(|_| None).unwrap().is_none());
        for name in [
            "CANTOS_PUBLICATION_ROOT",
            "CANTOS_PUBLIC_MEDIA_ORIGIN",
            "CANTOS_PLAYBACK_KEY_FILE",
        ] {
            assert!(matches!(
                from_lookup(|key| (key == name).then(|| "configured".into())),
                Err(PublicMediaConfigError::Incomplete)
            ));
        }
    }
    #[cfg(unix)]
    #[test]
    fn key_loader_rejects_permissions_lengths_symlinks_and_zero_key_without_exposing_bytes() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("cantos-public-key-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("key");
        for (bytes, mode, valid) in [
            (vec![7; 32], 0o600, true),
            (vec![7; 32], 0o400, true),
            (vec![7; 32], 0o644, false),
            (vec![7; 31], 0o600, false),
            (vec![7; 33], 0o600, false),
            (vec![0; 32], 0o600, false),
        ] {
            if path.exists() {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            }
            std::fs::write(&path, bytes).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            assert_eq!(read_key(&path).is_ok(), valid);
        }
        let link = root.join("alias");
        symlink(&path, &link).unwrap();
        assert!(matches!(
            read_key(&link),
            Err(PublicMediaConfigError::InvalidKey)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
