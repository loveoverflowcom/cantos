//! Process-level single ownership of one database file.
//!
//! The engine opens the same path twice in one process and does not stop a second process, so
//! ownership is a non-blocking advisory lock on a sidecar file. The lock is held by the open
//! file description: it ends with the process (even `kill -9`), leaving no stale pid to judge.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::error::{DirectoryFault, OpenError};

pub(crate) struct OwnerLock {
    _file: File,
}

pub(crate) fn lock_path(database: &Path) -> PathBuf {
    let mut name = database.as_os_str().to_owned();
    name.push(".owner");
    PathBuf::from(name)
}

/// The directory that will hold `file`, which must exist and (on Unix) grant nothing to its
/// group or others.
pub(crate) fn private_directory_of(file: &Path) -> Result<(PathBuf, fs::Metadata), OpenError> {
    let directory = match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let meta = fs::metadata(directory).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => OpenError::Directory(DirectoryFault::Missing),
        kind => OpenError::Io(kind),
    })?;
    if !meta.is_dir() {
        return Err(OpenError::Directory(DirectoryFault::NotDirectory));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.mode() & 0o077 != 0 {
            return Err(OpenError::Directory(DirectoryFault::TooPermissive));
        }
    }
    Ok((directory.to_path_buf(), meta))
}

impl OwnerLock {
    /// Refuses a missing, shared or foreign-owned directory and any symbolic-link target before
    /// the engine is allowed to create or open a file there.
    pub(crate) fn acquire(database: &Path) -> Result<Self, OpenError> {
        let (_, directory_meta) = private_directory_of(database)?;
        let lock = lock_path(database);
        for path in [database, lock.as_path()] {
            if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
                return Err(OpenError::Directory(DirectoryFault::SymlinkRefused));
            }
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(&lock)
            .map_err(|error| OpenError::Io(error.kind()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // A file we just opened belongs to our effective user; the directory must too.
            let ours = file
                .metadata()
                .map_err(|error| OpenError::Io(error.kind()))?
                .uid();
            if directory_meta.uid() != ours {
                return Err(OpenError::Directory(DirectoryFault::ForeignOwner));
            }
        }
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(OpenError::AlreadyOwned),
            Err(TryLockError::Error(error)) => Err(OpenError::Io(error.kind())),
        }
    }
}
